use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct ResetTrigger {
    pub id: i64,
    pub user_id: Option<i32>,
    pub token: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResetTriggerRequest {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResetTriggerRequest {
    pub password: String,
    pub password_confirmation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResetTriggerResponse {
    pub message: String,
}

impl ResetTrigger {
    pub fn to_response(&self) -> ResetTriggerResponse {
        ResetTriggerResponse {
            message: "If an account with that email exists, a password reset link has been sent.".to_string(),
        }
    }

    pub fn generate_token() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    pub fn generate_expires_at() -> String {
        let expires = Utc::now() + chrono::Duration::hours(24);
        expires.to_rfc3339()
    }

    pub fn is_expired(expires_at: Option<&str>) -> bool {
        match expires_at {
            None => true,
            Some(s) => {
                let parsed = chrono::DateTime::parse_from_rfc3339(s);
                match parsed {
                    Ok(dt) => dt.with_timezone(&Utc) < Utc::now(),
                    Err(_) => true,
                }
            }
        }
    }

    pub async fn find_by_id(pool: &sqlx::PgPool, id: i64) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Self>(
            "SELECT id, email, user_id, full_name, token, expires_at, created_at, updated_at
             FROM reset_triggers WHERE id = $1",
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
    fn reset_trigger_generate_token_is_uuid_format() {
        let token = ResetTrigger::generate_token();
        let uuid: Result<uuid::Uuid, _> = token.parse();
        assert!(uuid.is_ok(), "token should be valid UUID: {}", token);
        assert_eq!(token.len(), 36);
    }

    #[test]
    fn reset_trigger_generate_tokens_are_unique() {
        let token1 = ResetTrigger::generate_token();
        let token2 = ResetTrigger::generate_token();
        assert_ne!(token1, token2);
    }

    #[test]
    fn reset_trigger_generate_expires_at() {
        let expires = ResetTrigger::generate_expires_at();
        assert!(!expires.is_empty());
        assert!(expires.contains('T'));
    }

    #[test]
    fn reset_trigger_is_expired_returns_true_for_past() {
        let past = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        assert!(ResetTrigger::is_expired(Some(&past)));
    }

    #[test]
    fn reset_trigger_is_expired_returns_false_for_future() {
        let future = (Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        assert!(!ResetTrigger::is_expired(Some(&future)));
    }

    #[test]
    fn reset_trigger_is_expired_returns_true_for_none() {
        assert!(ResetTrigger::is_expired(None));
    }

    #[test]
    fn create_reset_trigger_request_serialization() {
        let req = CreateResetTriggerRequest {
            email: "user@example.com".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("user@example.com"));
    }

    #[test]
    fn update_reset_trigger_request_serialization() {
        let req = UpdateResetTriggerRequest {
            password: "newpassword123".to_string(),
            password_confirmation: "newpassword123".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("newpassword123"));
        assert!(json.contains("password_confirmation"));
    }
}
