use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct ShopifyJsonCache {
    pub id: i64,
    pub cached_item_id: Option<String>,
    pub json_entry: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShopifyJsonCacheResponse {
    pub id: i64,
    pub cached_item_id: Option<String>,
    pub json_entry: Option<String>,
}

impl ShopifyJsonCache {
    pub fn to_response(&self) -> ShopifyJsonCacheResponse {
        ShopifyJsonCacheResponse {
            id: self.id,
            cached_item_id: self.cached_item_id.clone(),
            json_entry: self.json_entry.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shopify_json_cache_to_response() {
        let cache = ShopifyJsonCache {
            id: 1,
            cached_item_id: Some("product-123".to_string()),
            json_entry: Some(r#"{"name":"Test"}"#.to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = cache.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.cached_item_id, Some("product-123".to_string()));
    }
}
