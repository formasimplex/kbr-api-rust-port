//! Shopify GraphQL client
//!
//! Provides a thin HTTP wrapper for Shopify Admin GraphQL API calls. Reads
//! `SHOPIFY_STORE` and `SHOPIFY_ACCESS_TOKEN` from environment. Construction
//! is optional — returns `None` if either env var is missing so the server
//! can start without Shopify configured.

use reqwest::Client as HttpClient;

/// Shopify Admin GraphQL client.
///
/// Holds the HTTP client, store URL, and access token. Use `new_from_env()`
/// for optional construction that won't fail if Shopify isn't configured.
#[derive(Clone)]
pub struct ShopifyClient {
    http: HttpClient,
    base_url: String,
    access_token: String,
}

impl ShopifyClient {
    /// Create from environment variables.
    ///
    /// Returns `None` if `SHOPIFY_STORE` or `SHOPIFY_ACCESS_TOKEN` is not set.
    pub fn new_from_env() -> Option<Self> {
        let shop = std::env::var("SHOPIFY_STORE").ok()?;
        let token = std::env::var("SHOPIFY_ACCESS_TOKEN").ok()?;
        Self::new(&shop, &token)
    }

    /// Create with explicit store domain and access token.
    ///
    /// # Arguments
    ///
    /// * `store` — Shopify store domain (e.g. `"mystore.myshopify.com"`)
    /// * `access_token` — Admin API access token
    pub fn new(store: &str, access_token: &str) -> Option<Self> {
        let http = HttpClient::builder()
            .user_agent("kbr-api-rust/1.0")
            .build()
            .ok()?;

        let base_url = format!("https://{}/admin/api/2024-01/graphql.json", store);

        Some(Self {
            http,
            base_url,
            access_token: access_token.to_string(),
        })
    }

    /// Execute a GraphQL query or mutation.
    ///
    /// Sends a POST request with the query string and optional variables.
    /// Returns the parsed JSON response body.
    ///
    /// # Arguments
    ///
    /// * `query` — The GraphQL query or mutation string
    /// * `variables` — JSON object of variables (use `serde_json::json!({})` for none)
    ///
    /// # Errors
    ///
    /// Returns `AppError::Shopify` if the HTTP request fails, the response
    /// status is not 200, or the response body isn't valid JSON.
    pub async fn query(
        &self,
        query: &str,
        variables: serde_json::Value,
    ) -> Result<serde_json::Value, crate::error::AppError> {
        let body = serde_json::json!({
            "query": query,
            "variables": variables,
        });

        let resp = self
            .http
            .post(&self.base_url)
            .header("X-Shopify-Access-Token", &self.access_token)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::error::AppError::Shopify(format!("Request failed: {}", e)))?;

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| crate::error::AppError::Shopify(format!("Failed to read response: {}", e)))?;

        if !status.is_success() {
            return Err(crate::error::AppError::Shopify(format!(
                "HTTP {}: {}",
                status, text
            )));
        }

        serde_json::from_str(&text)
            .map_err(|e| crate::error::AppError::Shopify(format!("Invalid JSON response: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_from_env_returns_none_when_missing() {
        unsafe {
            std::env::remove_var("SHOPIFY_STORE");
            std::env::remove_var("SHOPIFY_ACCESS_TOKEN");
        }
        assert!(ShopifyClient::new_from_env().is_none());
    }

    #[test]
    fn new_from_env_returns_some_when_set() {
        unsafe {
            std::env::set_var("SHOPIFY_STORE", "test.myshopify.com");
            std::env::set_var("SHOPIFY_ACCESS_TOKEN", "test-token-123");
        }
        let client = ShopifyClient::new_from_env();
        assert!(client.is_some());
    }

    #[test]
    fn new_constructs_correct_base_url() {
        let client = ShopifyClient::new("mystore.myshopify.com", "tok").unwrap();
        assert_eq!(
            client.base_url,
            "https://mystore.myshopify.com/admin/api/2024-01/graphql.json"
        );
    }

    #[test]
    fn new_from_env_missing_store() {
        unsafe {
            std::env::remove_var("SHOPIFY_STORE");
            std::env::set_var("SHOPIFY_ACCESS_TOKEN", "tok");
        }
        assert!(ShopifyClient::new_from_env().is_none());
    }

    #[test]
    fn new_from_env_missing_token() {
        unsafe {
            std::env::set_var("SHOPIFY_STORE", "test.myshopify.com");
            std::env::remove_var("SHOPIFY_ACCESS_TOKEN");
        }
        assert!(ShopifyClient::new_from_env().is_none());
    }

    #[test]
    fn client_is_clone() {
        unsafe {
            std::env::set_var("SHOPIFY_STORE", "test.myshopify.com");
            std::env::set_var("SHOPIFY_ACCESS_TOKEN", "tok");
        }
        let client = ShopifyClient::new_from_env().unwrap();
        let _clone = client.clone();
    }
}
