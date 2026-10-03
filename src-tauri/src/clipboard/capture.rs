use clipboard_rs::common::{ContentFormat, RustImage, RustImageData};
use clipboard_rs::{Clipboard, ClipboardContext};

/// Everything the pasteboard currently offers, read once per change.
///
/// The picture is carried as whatever `clipboard-rs` already decoded it into,
/// never as encoded bytes. Encoding here would cost a full-frame colour
/// conversion plus a PNG encode just to hand the pipeline something whose only
/// consumer decodes it straight back into pixels again.
///
/// Keeping it unencoded is also what keeps the snapshot cheap, and being cheap
/// is the whole point: this runs inside a callback that `clipboard-rs` invokes
/// serially on its own watcher thread, so time spent here is time during which
/// the next change to the system clipboard is not being noticed.
pub struct RawSnapshot {
    pub text: Option<String>,
    pub html: Option<String>,
    pub files: Vec<String>,
    pub image: Option<RustImageData>,
}

/// Reads the pasteboard, preferring the richest representation available.
///
/// Order matters: a Finder file copy also exposes a text flavour, and a
/// browser image copy exposes HTML + text too, so files win over images,
/// and images win over text.
pub fn snapshot(ctx: &ClipboardContext) -> RawSnapshot {
    let mut snap = RawSnapshot {
        text: None,
        html: None,
        files: Vec::new(),
        image: None,
    };

    if ctx.has(ContentFormat::Files) {
        if let Ok(files) = ctx.get_files() {
            snap.files = files;
        }
    }

    if snap.files.is_empty() && ctx.has(ContentFormat::Image) {
        if let Ok(image) = ctx.get_image() {
            // An empty frame can come back alongside a non-nil one, and an
            // empty one has no pixels to hash or write.
            snap.image = Some(image).filter(|img| !img.is_empty());
        }
    }

    if ctx.has(ContentFormat::Html) {
        snap.html = ctx.get_html().ok().filter(|s| !s.trim().is_empty());
    }

    if ctx.has(ContentFormat::Text) {
        snap.text = ctx.get_text().ok();
    }

    snap
}
