use blake3::Hasher;

fn fingerprint(parts: &[&[u8]]) -> String {
    let mut hasher = Hasher::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().to_hex().to_string()
}

/// Line endings are normalised; internal whitespace is significant.
pub fn text(value: &str) -> String {
    let normalised = value.replace("\r\n", "\n").replace('\r', "\n");
    fingerprint(&[b"text:", normalised.as_bytes()])
}

pub fn link(value: &str) -> String {
    let normalised = normalise_url(value.trim());
    fingerprint(&[b"link:", normalised.as_bytes()])
}

pub fn color(value: &str) -> String {
    let upper = value.trim().to_ascii_uppercase();
    fingerprint(&[b"color:", upper.as_bytes()])
}

/// Path order should not affect identity.
pub fn files(paths: &[String]) -> String {
    let mut sorted: Vec<&str> = paths.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    fingerprint(&[b"file:", sorted.join("\n").as_bytes()])
}

/// Lower-cases the scheme and host, and drops a trailing slash.
fn normalise_url(url: &str) -> String {
    let mut out = url.to_string();

    if let Some(idx) = out.find("://") {
        let (scheme, rest) = out.split_at(idx + 3);
        let scheme = scheme.to_ascii_lowercase();
        let host_end = rest.find('/').unwrap_or(rest.len());
        let (host, path) = rest.split_at(host_end);
        out = format!("{scheme}{}{path}", host.to_ascii_lowercase());
    }

    while out.ends_with('/') && out.len() > 9 {
        out.pop();
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_ignores_line_ending_style() {
        assert_eq!(text("a\r\nb"), text("a\nb"));
    }

    #[test]
    fn url_normalisation_lowercases_host_and_scheme() {
        assert_eq!(
            link("HTTPS://Example.COM/Path/"),
            link("https://example.com/Path")
        );
    }

    #[test]
    fn file_order_does_not_matter() {
        let a = vec!["/a".to_string(), "/b".to_string()];
        let b = vec!["/b".to_string(), "/a".to_string()];
        assert_eq!(files(&a), files(&b));
    }
}
