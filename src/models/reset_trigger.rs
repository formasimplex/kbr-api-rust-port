use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct ResetTrigger {
    pub id: i64,
    pub email: Option<String>,
    pub user_id: Option<i32>,
    pub full_name: Option<String>,
    pub token: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateResetTriggerRequest {
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateResetTriggerRequest {
    pub password: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResetTriggerResponse {
    pub id: i64,
    pub user_id: Option<i32>,
    pub token: Option<String>,
    pub expires_at: Option<String>,
}

impl ResetTrigger {
    pub fn to_response(&self) -> ResetTriggerResponse {
        ResetTriggerResponse {
            id: self.id,
            user_id: self.user_id,
            token: self.token.clone(),
            expires_at: self.expires_at.clone(),
        }
    }

    pub fn generate_token() -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..32)
            .map(|_| rng.gen_range(b'a'..=b'z'))
            .map(char::from)
            .collect()
    }

    pub fn generate_expires_at() -> String {
        let expires = Utc::now() + chrono::Duration::days(7);
        expires.format("%b %d, %Y %I:%M %p").to_string()
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
    fn reset_trigger_generate_token() {
        let token = ResetTrigger::generate_token();
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_lowercase()));
    }

    #[test]
    fn reset_trigger_generate_expires_at() {
        let expires = ResetTrigger::generate_expires_at();
        assert!(!expires.is_empty());
        assert!(expires.contains(", "));
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
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("newpassword123"));
    }
}
