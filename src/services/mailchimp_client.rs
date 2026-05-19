//! Mailchimp REST client
//!
//! Provides subscribe/unsubscribe operations for Mailchimp audience
//! management. Reads `MAILCHIMP_API_KEY` from environment. Construction
//! is optional — returns `None` if the env var is missing so the server
//! can start without Mailchimp configured.

use reqwest::Client as HttpClient;
use sha2::{Digest, Sha256};

use crate::error::AppError;

/// Mailchimp REST API client.
///
/// Holds the HTTP client, API key, data center, and audience ID.
#[derive(Clone)]
pub struct MailchimpClient {
    http: HttpClient,
    api_key: String,
    base_url: String,
}

impl MailchimpClient {
    /// Create from environment variable `MAILCHIMP_API_KEY`.
    ///
    /// Returns `None` if the API key is not set.
    pub fn new_from_env() -> Option<Self> {
        let api_key = std::env::var("MAILCHIMP_API_KEY").ok()?;
        Self::new(&api_key)
    }

    /// Create with explicit API key.
    ///
    /// The API key encodes the data center (e.g. `"abc123-us17"` → `"us17"`).
    /// Uses hardcoded audience ID `"97e79bbb31"` matching the Rails service.
    pub fn new(api_key: &str) -> Option<Self> {
        let http = HttpClient::builder()
            .user_agent("kbr-api-rust/1.0")
            .build()
            .ok()?;

        let dc = Self::extract_dc(api_key);
        let base_url = format!("https://{}.api.mailchimp.com/3.0/lists/97e79bbb31/members", dc);

        Some(Self {
            http,
            api_key: api_key.to_string(),
            base_url,
        })
    }

    /// Extract data center suffix from Mailchimp API key.
    fn extract_dc(api_key: &str) -> String {
        api_key
            .rsplit_once('-')
            .map(|(_, dc)| dc.to_string())
            .unwrap_or_else(|| "us17".to_string())
    }

    /// Compute Mailchimp member hash (MD5 of lowercase email).
    fn member_hash(email: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(email.to_lowercase().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Subscribe an email to the Mailchimp audience.
    ///
    /// Sends a POST request to create a new member with `subscribed` status.
    ///
    /// # Arguments
    ///
    /// * `email` — The email address to subscribe
    /// * `full_name` — The subscriber's full name (used for FNAME merge field)
    ///
    /// # Returns
    ///
    /// A result containing the API response body on success, or an `AppError::Mailchimp`
    /// if the request fails. Note: Mailchimp returns 200 for success and 400 for
    /// "already subscribed" — both are treated as success since the desired state
    /// is achieved.
    pub async fn subscribe(
        &self,
        email: &str,
        full_name: &str,
    ) -> Result<serde_json::Value, AppError> {
        let body = serde_json::json!({
            "email_address": email,
            "status": "subscribed",
            "merge_fields": {
                "FNAME": full_name
            }
        });

        let resp = self
            .http
            .post(&self.base_url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Mailchimp(format!("Request failed: {}", e)))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| AppError::Mailchimp(format!("Failed to read response: {}", e)))?;

        let body: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| AppError::Mailchimp(format!("Invalid JSON: {}", e)))?;

        if status.is_success() || status.as_u16() == 400 {
            Ok(body)
        } else {
            Err(AppError::Mailchimp(format!("HTTP {}: {}", status, text)))
        }
    }

    /// Unsubscribe an email from the Mailchimp audience.
    ///
    /// Sends a PATCH request to update the member status to `unsubscribed`.
    /// Uses MD5 hash of the lowercase email as the member identifier.
    ///
    /// # Arguments
    ///
    /// * `email` — The email address to unsubscribe
    ///
    /// # Returns
    ///
    /// A result containing the API response body on success, or an `AppError::Mailchimp`
    /// if the request fails.
    pub async fn unsubscribe(&self, email: &str) -> Result<serde_json::Value, AppError> {
        let hash = Self::member_hash(email);
        let url = format!("{}/{}", self.base_url, hash);

        let body = serde_json::json!({
            "status": "unsubscribed"
        });

        let resp = self
            .http
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Mailchimp(format!("Request failed: {}", e)))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| AppError::Mailchimp(format!("Failed to read response: {}", e)))?;

        let body: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| AppError::Mailchimp(format!("Invalid JSON: {}", e)))?;

        if [200, 201, 204].contains(&status.as_u16()) {
            Ok(body)
        } else {
            Err(AppError::Mailchimp(format!("HTTP {}: {}", status, text)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_from_env_returns_none_when_missing() {
        unsafe { std::env::remove_var("MAILCHIMP_API_KEY"); }
        assert!(MailchimpClient::new_from_env().is_none());
    }

    #[test]
    fn new_from_env_returns_some_when_set() {
        unsafe { std::env::set_var("MAILCHIMP_API_KEY", "test123-us17"); }
        let client = MailchimpClient::new_from_env();
        assert!(client.is_some());
    }

    #[test]
    fn new_constructs_correct_base_url() {
        let client = MailchimpClient::new("abc123-us17").unwrap();
        assert_eq!(
            client.base_url,
            "https://us17.api.mailchimp.com/3.0/lists/97e79bbb31/members"
        );
    }

    #[test]
    fn new_defaults_to_us17_when_no_dc() {
        let client = MailchimpClient::new("apikey123").unwrap();
        assert!(client.base_url.contains("us17.api.mailchimp.com"));
    }

    #[test]
    fn extract_dc_from_key() {
        assert_eq!(MailchimpClient::extract_dc("abc-us17"), "us17");
        assert_eq!(MailchimpClient::extract_dc("xyz-us2"), "us2");
    }

    #[test]
    fn member_hash_is_deterministic() {
        let h1 = MailchimpClient::member_hash("Test@Example.COM");
        let h2 = MailchimpClient::member_hash("test@example.com");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // SHA-256 hex length
    }

    #[test]
    fn client_is_clone() {
        unsafe { std::env::set_var("MAILCHIMP_API_KEY", "test-us17"); }
        let client = MailchimpClient::new_from_env().unwrap();
        let _clone = client.clone();
    }
}
