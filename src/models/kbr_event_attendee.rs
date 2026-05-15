use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct KbrEventAttendee {
    pub id: i64,
    pub kbr_event_id: Option<i32>,
    pub mail_subscriber_id: Option<i32>,
    pub scan_count: Option<i32>,
    pub headcount: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEventAttendeeRequest {
    pub kbr_event_id: i32,
    pub mail_subscriber_ids: Vec<i32>,
    pub headcount: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEventAttendeeRequest {
    pub kbr_event_id: i32,
    pub text_copy: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct KbrEventAttendeeResponse {
    pub id: i64,
    pub kbr_event_id: Option<i32>,
    pub mail_subscriber_id: Option<i32>,
    pub scan_count: Option<i32>,
    pub headcount: Option<i32>,
}

impl KbrEventAttendee {
    pub fn to_response(&self) -> KbrEventAttendeeResponse {
        KbrEventAttendeeResponse {
            id: self.id,
            kbr_event_id: self.kbr_event_id,
            mail_subscriber_id: self.mail_subscriber_id,
            scan_count: self.scan_count,
            headcount: self.headcount,
        }
    }

    pub fn has_scanned(&self) -> bool {
        self.scan_count.unwrap_or(0) > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kbr_event_attendee_not_scanned() {
        let attendee = KbrEventAttendee {
            id: 2,
            kbr_event_id: Some(5),
            mail_subscriber_id: Some(11),
            scan_count: Some(0),
            headcount: Some(1),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(!attendee.has_scanned());
    }
}
