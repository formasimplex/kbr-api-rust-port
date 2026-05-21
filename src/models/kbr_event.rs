use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::models::comment::CommentResponse;
use crate::utils::url::validate_url;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct KbrEvent {
    pub id: i64,
    pub name: Option<String>,
    pub description: Option<String>,
    pub active: Option<bool>,
    pub event_start_date: Option<DateTime<Utc>>,
    pub event_end_date: Option<DateTime<Utc>>,
    pub create_by_user_id: Option<i32>,
    pub event_url: Option<String>,
    pub qr_encode_string: Option<String>,
    pub ticket_url: Option<String>,
    pub external_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateKbrEventRequest {
    pub name: String,
    pub description: String,
    pub event_start_date: DateTime<Utc>,
    pub event_end_date: DateTime<Utc>,
    pub event_url: Option<String>,
    pub qr_encode_string: Option<String>,
    pub ticket_url: Option<String>,
    pub external_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateKbrEventRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub active: Option<bool>,
    pub event_start_date: Option<DateTime<Utc>>,
    pub event_end_date: Option<DateTime<Utc>>,
    pub ticket_url: Option<String>,
    pub external_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct KbrEventResponse {
    pub id: i64,
    pub name: Option<String>,
    pub description: Option<String>,
    pub active: Option<bool>,
    pub event_start_date: Option<DateTime<Utc>>,
    pub event_end_date: Option<DateTime<Utc>>,
    pub ticket_url: Option<String>,
    pub external_url: Option<String>,
    pub image_urls: Vec<String>,
    pub image_thumbnail_urls: Vec<String>,
    pub comments: Vec<CommentResponse>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl KbrEvent {
    pub fn to_response(
        &self,
        image_urls: Vec<String>,
        image_thumbnail_urls: Vec<String>,
        comments: Vec<CommentResponse>,
    ) -> KbrEventResponse {
        KbrEventResponse {
            id: self.id,
            name: self.name.clone(),
            description: Self::clean_description(&self.description),
            active: self.active,
            event_start_date: self.event_start_date,
            event_end_date: self.event_end_date,
            ticket_url: self.ticket_url.clone(),
            external_url: self.external_url.clone(),
            image_urls,
            image_thumbnail_urls,
            comments,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    fn clean_description(desc: &Option<String>) -> Option<String> {
        desc.as_ref().map(|d| d.replace(['\r', '\n'], ""))
    }
    pub fn validate_url(url: &str) -> bool {
        validate_url(url)
    }

    pub fn validate_required(name: &str, description: &str) -> bool {
        !name.is_empty() && !description.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_required_all_present() {
        assert!(KbrEvent::validate_required("Name", "Desc"));
    }

    #[test]
    fn validate_required_missing_fields() {
        assert!(!KbrEvent::validate_required("", "Desc"));
        assert!(!KbrEvent::validate_required("Name", ""));
    }
}
