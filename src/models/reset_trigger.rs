use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct ResetTrigger {
    pub id: i64,
    pub user_id: Option<i32>,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_trigger_to_response() {
        let trigger = ResetTrigger {
            id: 1,
            user_id: Some(42),
            token: Some("reset-token-abc".to_string()),
            expires_at: Some("May 12, 2026 10:30 AM".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = trigger.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.user_id, Some(42));
        assert_eq!(resp.token, Some("reset-token-abc".to_string()));
    }

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
