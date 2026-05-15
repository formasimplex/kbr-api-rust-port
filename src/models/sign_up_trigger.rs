use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct SignUpTrigger {
    pub id: i64,
    pub email: Option<String>,
    pub token: Option<String>,
    pub expires_at: Option<String>,
    pub role: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSignUpTriggerRequest {
    pub email: String,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SignUpTriggerResponse {
    pub id: i64,
    pub email: Option<String>,
    pub token: Option<String>,
    pub expires_at: Option<String>,
    pub role: Option<String>,
}

impl SignUpTrigger {
    pub fn to_response(&self) -> SignUpTriggerResponse {
        SignUpTriggerResponse {
            id: self.id,
            email: self.email.clone(),
            token: self.token.clone(),
            expires_at: self.expires_at.clone(),
            role: self.role.clone(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at.is_none()
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
    fn sign_up_trigger_is_expired_when_no_expires_at() {
        let trigger = SignUpTrigger {
            id: 1,
            email: Some("test@example.com".to_string()),
            token: Some("token".to_string()),
            expires_at: None,
            role: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(trigger.is_expired());
    }

    #[test]
    fn sign_up_trigger_not_expired_when_has_expires_at() {
        let trigger = SignUpTrigger {
            id: 1,
            email: Some("test@example.com".to_string()),
            token: Some("token".to_string()),
            expires_at: Some("May 12, 2026 10:30 AM".to_string()),
            role: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(!trigger.is_expired());
    }

    #[test]
    fn generate_token_returns_32_chars() {
        let token = SignUpTrigger::generate_token();
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_lowercase()));
    }

    #[test]
    fn generate_expires_at_returns_formatted_string() {
        let expires = SignUpTrigger::generate_expires_at();
        assert!(!expires.is_empty());
        assert!(expires.contains(", "));
    }

    #[test]
    fn create_sign_up_trigger_request_serialization() {
        let req = CreateSignUpTriggerRequest {
            email: "invite@example.com".to_string(),
            role: Some("artist".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("invite@example.com"));
        assert!(json.contains("artist"));
    }
}
