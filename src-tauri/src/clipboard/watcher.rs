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

struct CaptureHandler {
    state: Arc<AppState>,
    ctx: ClipboardContext,
    jobs: Sender<Job>,
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
        }
    }
}

/// Stores one queued observation: classify, write images, touch the database.
///
/// Runs off the watcher's own thread, which is the entire point — see the note
/// on [`Job`] for what used to happen in its place.
fn process(app: &AppHandle, state: &Arc<AppState>, job: Job) -> Result<()> {
    let Job { snapshot, source } = job;

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
                .as_ref()
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
        ClipKind::Link => {
            let text = snapshot.text.clone().unwrap_or_default();
            content_html = snapshot.html.clone();
            content_text = Some(text.clone());
            hash::link(&text)
        }
        ClipKind::Color => {
            let text = snapshot.text.clone().unwrap_or_default().trim().to_string();
            content_text = Some(text.clone());
            hash::color(&text)
        }
        ClipKind::Text => {
            let text = snapshot
                .text
                .clone()
                .or_else(|| snapshot.html.clone())
                .unwrap_or_default();
            content_html = snapshot.html.clone();
            content_text = Some(text.clone());
            hash::text(&text)
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

fn run_worker(app: AppHandle, state: Arc<AppState>, jobs: Receiver<Job>) {
    // One at a time, and in the order they were observed. Handling two at once
    // would be unsound at the database level, and finishing them out of order
    // would make `created_at` lie about which copy came first — the list is
    // nothing but that ordering.
    for job in jobs {
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

            std::thread::Builder::new()
                .name("clipboard-persist".into())
                .spawn({
                    let (app, state) = (app.clone(), state.clone());
                    move || run_worker(app, state, inbox)
                })
                .ok();

            watcher.add_handler(CaptureHandler { state, ctx, jobs });
            // Held for the lifetime of the loop so the watcher stays alive.
            let _shutdown = watcher.get_shutdown_channel();
            watcher.start_watch();
        })
        .ok();
}
