use std::path::Path;

use crate::error::Result;

#[cfg(target_os = "macos")]
use crate::error::AppError;

/// The icon's long edge, in pixels.
///
/// The slot it fills is 16 CSS px, so this is an exact 2x asset for a Retina
/// panel and a graceful downscale on a 1x one. The size is clamped here rather
/// than left to AppKit because the representation it hands back carries
/// whatever resolution the `.icns` happened to hold, which for a modern app is
/// every size from 16 to 1024 — trusting it is how a handful of apps turns into
/// a few megabytes each.
const ICON_PX: u32 = 32;

/// Where one bundle identifier's icon lives, relative to the data dir.
///
/// The name is a hash of the bundle id rather than the id itself.
/// `bundleIdentifier` is whatever the application chose to call itself, and
/// "whatever an app declares" is untrusted input: interpolated into a path it
/// would be a traversal, and `com.apple.Safari/../../Library` is not a
/// contrived input so much as an absence of validation. Hashing does not
/// sanitise the string so much as make the whole question go away — no
/// combination of separators, `..` or NULs survives into a filename, because
/// none of them can reach it.
fn relative_icon_path(bundle: &str) -> String {
    let digest = blake3::hash(bundle.as_bytes()).to_hex().to_string();
    format!("icons/{}.png", &digest[..16])
}

/// The cached icon for `bundle`, resolved and written on first sight.
///
/// `None` means no icon could be had, and the caller falls back to its initial
/// tile. The ways that happens are all ordinary: no bundle id on the row,
/// LaunchServices not resolving the id to an application, AppKit declining to
/// re-encode the icon, or a write that failed. None of them are worth
/// distinguishing in the UI — the fallback looks the same in every case.
///
/// Resolved on read rather than on capture for two reasons. AppKit's icon
/// service wants the main thread, which is where a synchronous command runs
/// (wry tags its IPC delegate `MainThreadOnly`) and where the capture path
/// does *not* — `watcher::spawn` does its work on a `clipboard-persist`
/// worker. And a capture is not a moment anyone looks at: the icon only ever
/// appears in the detail pane, so resolving one the user may never select
/// spends main-thread time for a picture nobody sees.
///
/// Caching is a filesystem `exists` check, the same bargain `media::persist`
/// strikes for frames. Every resolve happens on the main thread, so two
/// concurrent calls for a new app cannot race, and an in-memory layer would
/// only add a lock and an invalidation story to save a `stat`.
pub fn resolve(data_dir: &Path, bundle: Option<&str>) -> Option<String> {
    let bundle = bundle?;
    let relative = relative_icon_path(bundle);
    let absolute = data_dir.join(&relative);
    if absolute.exists() {
        return Some(absolute.to_string_lossy().into_owned());
    }

    let png = render(bundle).ok()?;
    std::fs::create_dir_all(data_dir.join("icons")).ok()?;
    std::fs::write(&absolute, png).ok()?;
    Some(absolute.to_string_lossy().into_owned())
}

/// Renders one bundle identifier's icon as a PNG at [`ICON_PX`].
///
/// The TIFF round trip is AppKit's, deliberately: handing its output to
/// `image` would ask that crate to decode a format this project has no
/// business knowing, and PNG in and PNG out is the part already proven by
/// every captured frame. What `image` does do is clamp the size, which is the
/// one step AppKit will not do on request.
#[cfg(target_os = "macos")]
fn render(bundle: &str) -> Result<Vec<u8>> {
    use image::imageops::FilterType;
    use image::ImageFormat;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSBitmapImageRepPropertyKey, NSWorkspace,
    };
    use objc2_foundation::{NSDictionary, NSString};

    let workspace = NSWorkspace::sharedWorkspace();
    // A bundle id LaunchServices cannot place: the app is uninstalled, or the
    // id was never registered to begin with.
    let app = workspace
        .URLForApplicationWithBundleIdentifier(&NSString::from_str(bundle))
        .ok_or_else(|| AppError::Other(format!("no app registered for {bundle}")))?;
    let path = app
        .path()
        .ok_or_else(|| AppError::Other(format!("{bundle} resolved to no path")))?;

    // Not an `Option`, and not something to defend against: `iconForFile:`
    // substitutes a generic document icon rather than returning nil.
    let image = workspace.iconForFile(&path);
    let tiff = image
        .TIFFRepresentation()
        .ok_or_else(|| AppError::Other(format!("{bundle} icon had no TIFF form")))?;
    let rep = NSBitmapImageRep::imageRepWithData(&tiff)
        .ok_or_else(|| AppError::Other(format!("{bundle} icon TIFF was unreadable")))?;

    // SAFETY: the contract is that `properties` holds keys of the dictionary's
    // key type with values of its object type. An empty dictionary has neither,
    // so there is nothing that could be mistyped — which is also why PNG wants
    // no options set to keep it straight.
    let properties = NSDictionary::<NSBitmapImageRepPropertyKey, AnyObject>::new();
    let png =
        unsafe { rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &properties) }
            .ok_or_else(|| AppError::Other(format!("AppKit refused to encode {bundle}")))?;

    let decoded = image::load_from_memory_with_format(&png.to_vec(), ImageFormat::Png)?;
    let small = decoded.resize_exact(ICON_PX, ICON_PX, FilterType::Triangle);

    let mut out = std::io::Cursor::new(Vec::new());
    small.write_to(&mut out, ImageFormat::Png)?;
    Ok(out.into_inner())
}

#[cfg(not(target_os = "macos"))]
fn render(_bundle: &str) -> Result<Vec<u8>> {
    Err(crate::error::AppError::Other(
        "app icons are a macOS feature".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache keys off this string, so a value that moved would defeat the
    /// `exists` check above and rewrite the file on every single selection.
    #[test]
    fn the_path_is_stable_across_calls() {
        assert_eq!(
            relative_icon_path("com.apple.Safari"),
            relative_icon_path("com.apple.Safari")
        );
    }

    /// The reason the name is a hash. A bundle id that reaches a filesystem
    /// path unfiltered could walk out of the icons directory and into the rest
    /// of the data dir; these inputs are the shapes that would do it.
    ///
    /// The check is on the file name alone. The leading `icons/` is this
    /// module's own separator and has to be there — what must never appear is a
    /// separator or a parent reference *inside* the name, because those are
    /// what would let an input choose a directory other than `icons`.
    #[test]
    fn hostile_bundle_ids_cannot_escape_the_icons_directory() {
        for bundle in [
            "../../etc/passwd",
            "com.apple.Safari/../../Library",
            "com.apple.Safari/../../../.ssh/id_rsa",
            "",
            "/",
            "..",
        ] {
            let path = relative_icon_path(bundle);
            let name = path
                .strip_prefix("icons/")
                .expect("icons dir is the prefix");
            let name = name.strip_suffix(".png").expect("the name ends in .png");

            assert!(!name.contains(".."), "{bundle:?} leaked a parent ref");
            assert!(!name.contains('/'), "{bundle:?} leaked a separator");
            assert!(!name.contains('\\'), "{bundle:?} leaked a backslash");
            assert!(!name.contains('\0'), "{bundle:?} leaked a NUL");
        }
    }

    /// Case is significant: `com.google.Chrome` and `com.google.chrome` are
    /// different identifiers to AppKit, and folding them would point one
    /// application's row at another's icon.
    #[test]
    fn identifiers_differing_only_in_case_stay_separate() {
        assert_ne!(
            relative_icon_path("com.google.Chrome"),
            relative_icon_path("com.google.chrome")
        );
    }
}
