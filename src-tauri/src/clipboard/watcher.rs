use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;

use clipboard_rs::common::{ClipboardHandler, RustImage};
use clipboard_rs::{ClipboardContext, ClipboardWatcher, ClipboardWatcherContext};
use tauri::{AppHandle, Emitter};

use super::{capture, classify, hash};
use crate::error::{AppError, Result};
use crate::models::{AppInfo, ClipKind};
use crate::state::AppState;
use crate::store::repo::{self, NewClipping, UpsertOutcome};

/// One observation of the pasteboard, priced to be taken inside the callback.
///
/// Deliberately carries nothing expensive and nothing that needs deciding
/// later. See [`CaptureHandler::on_clipboard_change`] for why the cheapness is
/// load-bearing rather than tidiness.
struct Job {
    snapshot: capture::RawSnapshot,
    /// Read by the callback, not the worker: by the time the worker gets here
    /// the user may well have switched applications, and a clip filed under
    /// whatever they happened to be looking at next is simply wrong. Source
    /// attribution is only true at the instant of the copy.
    source: Option<AppInfo>,
}

/// Queue depth past which the "a burst resolves itself" assumption behind the
/// unbounded channel is worth a warning. Not a limit — nothing is dropped on
/// account of crossing it.
const QUEUE_WARN_DEPTH: usize = 32;

struct CaptureHandler {
    state: Arc<AppState>,
    ctx: ClipboardContext,
    jobs: Sender<Job>,
    /// Jobs handed to the worker but not yet taken off the queue.
    depth: Arc<AtomicUsize>,
    /// Latches the backlog warning so a sustained burst logs it once.
    warned: Arc<AtomicBool>,
}

impl ClipboardHandler for CaptureHandler {
    fn on_clipboard_change(&mut self) {
        // Consumed here rather than downstream, because this answers a
        // question only this vantage point can: *the change I am looking at*
        // — was it one of ours? Handed to the worker it would race against
        // every real copy the user makes while the queue drains.
        if self.state.take_suppress() {
            return;
        }

        let job = Job {
            snapshot: capture::snapshot(&self.ctx),
            source: crate::frontmost::remember_and_get(&self.state),
        };

        if self.jobs.send(job).is_err() {
            eprintln!("[clipboard] the persist worker is gone; dropping this change");
            return;
        }

        // The queue is unbounded on purpose (see `spawn`), on the assumption
        // that a burst resolves itself and that a backlog is essentially never
        // more than one or two deep. This makes that assumption observable
        // rather than merely asserted: past `QUEUE_WARN_DEPTH` the premise is
        // wrong — the worker is not keeping up and the queue is holding
        // decoded images — and we want to hear about it, once.
        let depth = self.depth.fetch_add(1, Ordering::Relaxed) + 1;
        if depth > QUEUE_WARN_DEPTH && !self.warned.swap(true, Ordering::Relaxed) {
            eprintln!(
                "[clipboard] persist backlog reached {depth} jobs; captures are \
                 outrunning the worker and the unbounded queue may be holding \
                 decoded images"
            );
        }
    }
}

/// Stores one queued observation: classify, write images, touch the database.
///
/// Runs off the watcher's own thread, which is the entire point — see the note
/// on [`Job`] for what used to happen in its place.
fn process(app: &AppHandle, state: &Arc<AppState>, job: Job) -> Result<()> {
    let Job {
        mut snapshot,
        source,
    } = job;

    let Some(kind) = classify::classify(&snapshot) else {
        return Ok(());
    };

    let mut content_text = None;
    let mut content_html = None;
    let mut image_path = None;
    let mut thumb_path = None;
    let mut width = None;
    let mut height = None;
    let mut byte_size = None;

    let hash = match kind {
        ClipKind::Image => {
            let image = snapshot
                .image
                .take()
                .ok_or_else(|| AppError::Other("image flavour was empty".into()))?;
            let stored = crate::media::persist(
                &state.data_dir,
                image
                    .to_rgba8()
                    .map_err(|e| AppError::Clipboard(e.to_string()))?,
            )?;
            image_path = Some(stored.image_rel);
            thumb_path = Some(stored.thumb_rel);
            width = Some(stored.width);
            height = Some(stored.height);
            byte_size = Some(stored.byte_size);
            stored.hash
        }
        ClipKind::File => {
            content_text = Some(snapshot.files.join("\n"));
            hash::files(&snapshot.files)
        }
        // The arms below *move* the snapshot's fields out rather than cloning
        // them: a large paste used to be copied two or three times over, per
        // capture, for no reason. Order matters in `Text` — `content_html`
        // has to take the html before the text fallback can reach for it.
        ClipKind::Link => {
            content_html = snapshot.html.take();
            let text = snapshot.text.take().unwrap_or_default();
            let hash = hash::link(&text);
            content_text = Some(text);
            hash
        }
        ClipKind::Color => {
            let text = snapshot.text.take().unwrap_or_default().trim().to_string();
            let hash = hash::color(&text);
            content_text = Some(text);
            hash
        }
        ClipKind::Text => {
            content_html = snapshot.html.take();
            let text = snapshot
                .text
                .take()
                .or_else(|| content_html.clone())
                .unwrap_or_default();
            let hash = hash::text(&text);
            content_text = Some(text);
            hash
        }
    };

    let clip = NewClipping {
        kind,
        content_text,
        content_html,
        image_path,
        thumb_path,
        width,
        height,
        byte_size,
        hash,
        source_app: source.as_ref().map(|a| a.name.clone()),
        source_bundle: source.as_ref().and_then(|a| a.bundle.clone()),
    };

    let outcome = state.db.with(|conn| repo::upsert(conn, &clip))?;

    if let UpsertOutcome::Inserted(id) = outcome {
        // LRU-evict beyond the configured ceiling.
        let max_items = state.settings.read().max_items;
        let evicted = state.db.with(|conn| repo::prune(conn, max_items))?;
        for rel in evicted {
            crate::media::remove(&state.data_dir, &rel);
        }
        println!("[clipboard] captured #{id} ({})", kind.as_str());
    } else if let UpsertOutcome::Bumped(id) = outcome {
        println!("[clipboard] bumped #{id}");
    }

    let _ = app.emit("clip://changed", ());
    Ok(())
}

fn run_worker(app: AppHandle, state: Arc<AppState>, jobs: Receiver<Job>, depth: Arc<AtomicUsize>) {
    // One at a time, and in the order they were observed. Handling two at once
    // would be unsound at the database level, and finishing them out of order
    // would make `created_at` lie about which copy came first — the list is
    // nothing but that ordering.
    for job in jobs {
        // Against the handler's backlog gauge, and decremented on arrival
        // rather than on completion: it measures how far the queue has grown,
        // not how long one capture takes to persist.
        depth.fetch_sub(1, Ordering::Relaxed);
        if let Err(err) = process(&app, &state, job) {
            eprintln!("[clipboard] capture failed: {err}");
        }
    }
}

/// Starts the watcher on a dedicated thread, and the queue that keeps its
/// callback cheap beside it.
///
/// `start_watch` blocks until the shutdown channel fires, so neither this nor
/// the worker may run on the main thread.
pub fn spawn(app: AppHandle, state: Arc<AppState>) {
    std::thread::Builder::new()
        .name("clipboard-watcher".into())
        .spawn(move || {
            let ctx = match ClipboardContext::new() {
                Ok(ctx) => ctx,
                Err(err) => {
                    eprintln!("[clipboard] could not open pasteboard: {err}");
                    return;
                }
            };

            let mut watcher = match ClipboardWatcherContext::new() {
                Ok(watcher) => watcher,
                Err(err) => {
                    eprintln!("[clipboard] could not start watcher: {err}");
                    return;
                }
            };

            // Every pair of brackets below is the point of this function. The
            // handler used to do the whole capture inline — read the board,
            // encode a PNG, decode it again, hash it, resize it, write two
            // files and two database rows. `clipboard-rs` calls that handler
            // *serially* from its own thread, so a 4K screenshot occupying a
            // few hundred milliseconds meant every subsequent change waited,
            // and rapid consecutive copies simply went unrecorded.
            //
            // Now the callback is bounded by reading the board and asking who
            // owns the front at that instant; the rest is queued.
            //
            // The channel is unbounded on purpose: dropping a capture because
            // a burst of them arrived would quietly lose user data, and the
            // producer's cost per item is small enough that a burst resolves
            // itself. What bounds it in practice is that the queue is empty
            // essentially always.
            let (jobs, inbox) = mpsc::channel::<Job>();

            // Shared with the handler so the queue's depth is observable; see
            // `QUEUE_WARN_DEPTH`. Nothing reads the gauge to make a decision —
            // it only ever produces a log line.
            let depth = Arc::new(AtomicUsize::new(0));
            let warned = Arc::new(AtomicBool::new(false));

            std::thread::Builder::new()
                .name("clipboard-persist".into())
                .spawn({
                    let (app, state, depth) = (app.clone(), state.clone(), depth.clone());
                    move || run_worker(app, state, inbox, depth)
                })
                .ok();

            watcher.add_handler(CaptureHandler {
                state,
                ctx,
                jobs,
                depth,
                warned,
            });
            // Held for the lifetime of the loop so the watcher stays alive.
            let _shutdown = watcher.get_shutdown_channel();
            watcher.start_watch();
        })
        .ok();
}
