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
        match &self.expires_at {
            None => true,
            Some(s) => match Self::parse_expires_at(s) {
                Some(dt) => dt < Utc::now(),
                None => true,
            },
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
        let expires = Utc::now() + chrono::Duration::days(1);
        expires.format("%b %d, %Y %I:%M %p").to_string()
    }

    pub(crate) fn parse_expires_at(s: &str) -> Option<DateTime<Utc>> {
        let dt = chrono::NaiveDateTime::parse_from_str(s, "%b %d, %Y %I:%M %p").ok()?;
        Some(dt.and_utc())
    }

    #[cfg(test)]
    pub fn parse_expires_at_for_test(s: Option<&str>) -> Option<DateTime<Utc>> {
        let s = s?;
        let dt = chrono::NaiveDateTime::parse_from_str(s, "%b %d, %Y %I:%M %p").ok()?;
        Some(dt.and_utc())
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
    fn sign_up_trigger_not_expired_when_future() {
        let future = (Utc::now() + chrono::Duration::days(1))
            .format("%b %d, %Y %I:%M %p")
            .to_string();
        let trigger = SignUpTrigger {
            id: 1,
            email: Some("test@example.com".to_string()),
            token: Some("token".to_string()),
            expires_at: Some(future),
            role: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(!trigger.is_expired());
    }

    #[test]
    fn sign_up_trigger_is_expired_when_past() {
        let past = (Utc::now() - chrono::Duration::hours(1))
            .format("%b %d, %Y %I:%M %p")
            .to_string();
        let trigger = SignUpTrigger {
            id: 1,
            email: Some("test@example.com".to_string()),
            token: Some("token".to_string()),
            expires_at: Some(past),
            role: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(trigger.is_expired());
    }

    #[test]
    fn sign_up_trigger_is_expired_on_parse_failure() {
        let trigger = SignUpTrigger {
            id: 1,
            email: Some("test@example.com".to_string()),
            token: Some("token".to_string()),
            expires_at: Some("not-a-date".to_string()),
            role: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(trigger.is_expired());
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
    fn generate_expires_at_is_one_day_out() {
        let expires_str = SignUpTrigger::generate_expires_at();
        let parsed: DateTime<Utc> = chrono::NaiveDateTime::parse_from_str(
            &expires_str,
            "%b %d, %Y %I:%M %p",
        )
        .unwrap()
        .and_utc();
        let diff = (parsed - Utc::now()).num_hours();
        assert!(diff >= 23, "Expected ~24 hours, got {}", diff);
        assert!(diff < 25, "Expected ~24 hours, got {}", diff);
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
