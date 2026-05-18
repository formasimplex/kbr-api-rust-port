/// Validates that a URL is non-empty, within length limit, uses http/https scheme,
/// and doesn't target local/cloud-metadata addresses (SSRF prevention).
pub fn validate_url(url: &str) -> bool {
    if url.is_empty() || url.len() > 255 {
        return false;
    }
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return false;
    }
    let dangerous = ["localhost", "127.0.0.1", "0.0.0.0", "169.254.169.254"];
    if dangerous.iter().any(|d| url.contains(d)) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_urls() {
        assert!(validate_url("https://example.com"));
        assert!(validate_url("http://example.com"));
        assert!(validate_url("http://example.com/path?query=1"));
    }

    #[test]
    fn invalid_urls() {
        assert!(!validate_url(""));
        assert!(!validate_url("ftp://example.com"));
        assert!(!validate_url("example.com"));
        assert!(!validate_url(&"a".repeat(256)));
    }
}
