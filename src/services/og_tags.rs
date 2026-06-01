use std::collections::HashMap;

use reqwest::Client;

/// Fetches a URL's HTML and extracts Open Graph meta tags.
/// Returns `None` on any error (fetch failure, non-200, parse error).
pub struct OgTagsService {
    http: Client,
}

impl Default for OgTagsService {
    fn default() -> Self {
        Self::new()
    }
}

impl OgTagsService {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .user_agent("kbr-api-rust/1.0")
                .build()
                .expect("Failed to build reqwest client"),
        }
    }

    pub async fn fetch(&self, url: &str) -> Option<HashMap<String, String>> {
        let resp = self.http.get(url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let html = resp.text().await.ok()?;
        parse_og_tags(&html)
    }
}

/// Extracts Open Graph meta tags from HTML content.
/// Returns a HashMap of property name (without "og:" prefix) to content value.
fn parse_og_tags(html: &str) -> Option<HashMap<String, String>> {
    let meta_tags = tagparser::parse_tags_with_attr(
        html.to_string(),
        "meta".to_string(),
        "property",
        None,
    );

    let mut tags = HashMap::new();

    for tag in meta_tags {
        let property = extract_attribute(&tag, "property")?;
        let content = extract_attribute(&tag, "content")?;
        if let Some(name) = property.strip_prefix("og:") {
            tags.insert(name.to_string(), content);
        }
    }

    Some(tags)
}

/// Extracts an attribute value from a single HTML tag string.
fn extract_attribute(tag: &str, attr: &str) -> Option<String> {
    let attr_pattern = format!("{}=", attr);
    let start = tag.find(&attr_pattern)? + attr_pattern.len();
    let rest = &tag[start..];

    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }

    let value_end = rest[1..].find(quote)? + 1;
    Some(rest[1..value_end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_og_tags_extracts_image_and_title() {
        let html = r#"
            <html><head>
            <meta property="og:title" content="My Article" />
            <meta property="og:image" content="https://example.com/img.jpg" />
            <meta property="og:description" content="A description" />
            </head></html>
        "#;
        let tags = parse_og_tags(html).unwrap();
        assert_eq!(tags.get("title"), Some(&"My Article".to_string()));
        assert_eq!(tags.get("image"), Some(&"https://example.com/img.jpg".to_string()));
        assert_eq!(tags.get("description"), Some(&"A description".to_string()));
    }

    #[test]
    fn parse_og_tags_ignores_non_og_meta() {
        let html = r#"<meta property="twitter:card" content="summary" />"#;
        let tags = parse_og_tags(html).unwrap();
        assert!(tags.is_empty());
    }

    #[test]
    fn parse_og_tags_empty_html_returns_empty_map() {
        let html = "<html></html>";
        let tags = parse_og_tags(html).unwrap();
        assert!(tags.is_empty());
    }

    #[test]
    fn extract_attribute_single_quotes() {
        let tag = r#"<meta property='og:title' content='Hello' />"#;
        assert_eq!(extract_attribute(&tag, "property"), Some("og:title".to_string()));
        assert_eq!(extract_attribute(&tag, "content"), Some("Hello".to_string()));
    }

    #[test]
    fn extract_attribute_missing() {
        let tag = r#"<meta property="og:title" />"#;
        assert_eq!(extract_attribute(&tag, "content"), None);
    }

    #[test]
    fn og_tags_service_new() {
        let _service = OgTagsService::new();
    }
}
