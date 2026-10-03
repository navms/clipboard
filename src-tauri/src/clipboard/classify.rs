use super::capture::RawSnapshot;
use crate::models::ClipKind;

/// Maps a raw pasteboard snapshot onto the app's clip kinds.
/// Returns `None` when there is nothing worth recording.
pub fn classify(snap: &RawSnapshot) -> Option<ClipKind> {
    if !snap.files.is_empty() {
        return Some(ClipKind::File);
    }
    if snap.image.is_some() {
        return Some(ClipKind::Image);
    }
    if let Some(text) = snap.text.as_deref() {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            if is_color(trimmed) {
                return Some(ClipKind::Color);
            }
            if is_link(trimmed) {
                return Some(ClipKind::Link);
            }
            return Some(ClipKind::Text);
        }
    }
    if snap.html.as_deref().is_some_and(|h| !h.trim().is_empty()) {
        return Some(ClipKind::Text);
    }
    None
}

/// `#abc`, `#aabbcc`, `rgb(...)`, `rgba(...)`, `hsl(...)`: a whole-string match,
/// not a substring one, so a paragraph containing "#fff" stays plain text.
pub fn is_color(value: &str) -> bool {
    if value.contains(char::is_whitespace) {
        return false;
    }

    if let Some(hex) = value.strip_prefix('#') {
        let len = hex.len();
        return (len == 3 || len == 4 || len == 6 || len == 8)
            && hex.chars().all(|c| c.is_ascii_hexdigit());
    }

    let lower = value.to_ascii_lowercase();
    for prefix in ["rgb(", "rgba(", "hsl(", "hsla("] {
        if lower.starts_with(prefix) && lower.ends_with(')') {
            return true;
        }
    }

    false
}

/// A single whitespace-free token with an http(s) scheme.
pub fn is_link(value: &str) -> bool {
    if value.contains(char::is_whitespace) {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    (lower.starts_with("http://") || lower.starts_with("https://"))
        && value.len() > "https://".len()
}
