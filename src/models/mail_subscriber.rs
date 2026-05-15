use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct MailSubscriber {
    pub id: i64,
    pub full_name: String,
    pub email: String,
    pub active: Option<bool>,
    pub artist_id: Option<i64>,
    pub unsubscribed_at: Option<DateTime<Utc>>,
    pub unsubscribe_token: Option<String>,
    pub user_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMailSubscriberRequest {
    pub full_name: String,
    pub email: String,
    pub artist_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MailSubscriberResponse {
    pub id: i64,
    pub full_name: String,
    pub email: String,
    pub active: Option<bool>,
    pub artist_id: Option<i64>,
    pub user_id: Option<i64>,
}

impl MailSubscriber {
    pub fn to_response(&self) -> MailSubscriberResponse {
        MailSubscriberResponse {
            id: self.id,
            full_name: self.full_name.clone(),
            email: self.email.clone(),
            active: self.active,
            artist_id: self.artist_id,
            user_id: self.user_id,
        }
    }

    pub fn is_subscribed(&self) -> bool {
        self.unsubscribed_at.is_none()
    }

    pub fn validate_email(email: &str) -> bool {
        !email.is_empty() && email.contains('@')
    }

    pub fn generate_unsubscribe_token() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..64)
            .map(|_| rng.gen_range(b'a'..=b'z'))
            .map(char::from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_subscriber_unsubscribed() {
        let sub = MailSubscriber {
            id: 2,
            full_name: "Jane".to_string(),
            email: "jane@example.com".to_string(),
            active: Some(false),
            artist_id: None,
            unsubscribed_at: Some(Utc::now()),
            unsubscribe_token: None,
            user_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(!sub.is_subscribed());
    }

    #[test]
    fn validate_email() {
        assert!(MailSubscriber::validate_email("test@example.com"));
        assert!(!MailSubscriber::validate_email(""));
        assert!(!MailSubscriber::validate_email("no-at"));
    }

    #[test]
    fn generate_unsubscribe_token() {
        let token = MailSubscriber::generate_unsubscribe_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_lowercase()));
    }
}
