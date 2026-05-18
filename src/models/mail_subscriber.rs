use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct MailSubscriber {
    pub id: i64,
    pub full_name: String,
    pub email: String,
    pub active: Option<bool>,
    pub artist_id: Option<i64>,
    pub unsubscribed_at: Option<DateTime<Utc>>,
    pub unsubscribe_token: Option<String>,
    pub unsubscribe_token_expires_at: Option<DateTime<Utc>>,
    pub user_id: Option<i64>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
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
        Uuid::new_v4().simple().to_string()
    }

    pub async fn find_by_id(pool: &sqlx::PgPool, id: i64) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Self>(
            "SELECT id, full_name, email, active, artist_id, unsubscribed_at,
                    unsubscribe_token, unsubscribe_token_expires_at, user_id, first_name, last_name, created_at, updated_at
             FROM mail_subscribers WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
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
            unsubscribe_token_expires_at: None,
            user_id: None,
            first_name: None,
            last_name: None,
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
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
