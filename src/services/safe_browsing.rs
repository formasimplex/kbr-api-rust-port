//! Google Safe Browsing client
//!
//! Checks URLs against Google's Safe Browsing API to detect malicious
//! content. Reads `GOOGLE_SAFE_BROWSING_API_KEY` from environment.
//! Construction is optional — returns `None` if the env var is missing.

use reqwest::Client as HttpClient;

/// Google Safe Browsing API client.
///
/// Holds the HTTP client and API key for threat lookup.
#[derive(Clone)]
pub struct SafeBrowsingClient {
    http: HttpClient,
    api_key: String,
}

impl SafeBrowsingClient {
    /// Create from environment variable `GOOGLE_SAFE_BROWSING_API_KEY`.
    ///
    /// Returns `None` if the API key is not set.
    pub fn new_from_env() -> Option<Self> {
        let api_key = std::env::var("GOOGLE_SAFE_BROWSING_API_KEY").ok()?;
        Self::new(&api_key)
    }

    /// Create with explicit API key.
    pub fn new(api_key: &str) -> Option<Self> {
        let http = HttpClient::builder()
            .user_agent("kbr-api-rust/1.0")
            .build()
            .ok()?;

        Some(Self {
            http,
            api_key: api_key.to_string(),
        })
    }

    /// Check whether a URL is malicious.
    ///
    /// Queries Google Safe Browsing v4 API for threat matches. Returns `true`
    /// if the URL matches any threat type, `false` if clean. On API errors,
    /// returns `false` (fail-open) to match Rails behavior.
    ///
    /// # Arguments
    ///
    /// * `url` — The URL to check
    ///
    /// # Returns
    ///
    /// `true` if the URL is potentially malicious, `false` otherwise.
    /// API failures are logged and treated as safe (fail-open).
    pub async fn is_malicious(&self, url: &str) -> bool {
        let body = serde_json::json!({
            "client": {
                "clientId": "formasimplex",
                "clientVersion": "1.0"
            },
            "threatInfo": {
                "threatTypes": [
                    "MALWARE",
                    "SOCIAL_ENGINEERING",
                    "UNWANTED_SOFTWARE",
                    "POTENTIALLY_HARMFUL_APPLICATION"
                ],
                "platformTypes": ["ANY_PLATFORM"],
                "threatEntryTypes": ["URL"],
                "threatEntries": [{ "url": url }]
            }
        });

        let url = format!(
            "https://safebrowsing.googleapis.com/v4/threatMatches:find?key={}",
            self.api_key
        );

        let resp = self.http.post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        let resp = match resp {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "Safe Browsing request failed, failing open");
                return false;
            }
        };

        if !resp.status().is_success() {
            tracing::warn!(status = %resp.status(), "Safe Browsing non-success response, failing open");
            return false;
        }

        let text = match resp.text().await {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(error = %e, "Safe Browsing failed to read response, failing open");
                return false;
            }
        };

        let body: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "Safe Browsing invalid JSON, failing open");
                return false;
            }
        };

        let is_malicious = body.get("matches").is_some();
        if is_malicious {
            tracing::warn!(url = %url, "URL flagged as potentially malicious");
        } else {
            tracing::info!(url = %url, "URL is safe");
        }
        is_malicious
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_from_env_returns_none_when_missing() {
        unsafe { std::env::remove_var("GOOGLE_SAFE_BROWSING_API_KEY"); }
        assert!(SafeBrowsingClient::new_from_env().is_none());
    }

    #[test]
    fn new_from_env_returns_some_when_set() {
        unsafe { std::env::set_var("GOOGLE_SAFE_BROWSING_API_KEY", "test-key-123"); }
        let client = SafeBrowsingClient::new_from_env();
        assert!(client.is_some());
    }

    #[test]
    fn new_constructs_client() {
        let client = SafeBrowsingClient::new("test-key").unwrap();
        assert_eq!(client.api_key, "test-key");
    }

    #[test]
    fn client_is_clone() {
        unsafe { std::env::set_var("GOOGLE_SAFE_BROWSING_API_KEY", "test-key"); }
        let client = SafeBrowsingClient::new_from_env().unwrap();
        let _clone = client.clone();
    }
}
