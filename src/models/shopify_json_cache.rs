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


