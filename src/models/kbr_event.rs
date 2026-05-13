use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl KbrEvent {
    pub fn to_response(&self) -> KbrEventResponse {
        KbrEventResponse {
            id: self.id,
            name: self.name.clone(),
            description: Self::clean_description(&self.description),
            active: self.active,
            event_start_date: self.event_start_date,
            event_end_date: self.event_end_date,
            ticket_url: self.ticket_url.clone(),
            external_url: self.external_url.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    fn clean_description(desc: &Option<String>) -> Option<String> {
        desc.as_ref().map(|d| d.replace(['\r', '\n'], ""))
    }

    pub fn validate_required(name: &str, description: &str, has_dates: bool, has_user: bool) -> bool {
        !name.is_empty() && !description.is_empty() && has_dates && has_user
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kbr_event_to_response() {
        let event = KbrEvent {
            id: 1,
            name: Some("Summer Festival".to_string()),
            description: Some("A great festival\r\nwith music".to_string()),
            active: Some(true),
            event_start_date: Some(Utc::now()),
            event_end_date: Some(Utc::now()),
            create_by_user_id: Some(5),
            event_url: None,
            qr_encode_string: None,
            ticket_url: Some("https://tickets.com/123".to_string()),
            external_url: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = event.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.name, Some("Summer Festival".to_string()));
        assert!(!resp.description.as_ref().unwrap().contains('\r'));
        assert!(!resp.description.as_ref().unwrap().contains('\n'));
    }

    #[test]
    fn validate_required_all_present() {
        assert!(KbrEvent::validate_required("Name", "Desc", true, true));
    }

    #[test]
    fn validate_required_missing_fields() {
        assert!(!KbrEvent::validate_required("", "Desc", true, true));
        assert!(!KbrEvent::validate_required("Name", "", true, true));
        assert!(!KbrEvent::validate_required("Name", "Desc", false, true));
        assert!(!KbrEvent::validate_required("Name", "Desc", true, false));
    }
}
