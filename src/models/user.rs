use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, FromRow)]
pub struct UserRow {
    pub id: i64,
    pub email: String,
    pub password_digest: String,
    pub role: Option<String>,
    pub session_token: Option<String>,
    pub username: Option<String>,
    pub token_version: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, FromRow)]
pub struct UserRowNoPassword {
    pub id: i64,
    pub email: String,
    pub role: Option<String>,
    pub session_token: Option<String>,
    pub username: Option<String>,
    pub token_version: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        User {
            id: row.id,
            email: row.email,
            password_digest: row.password_digest,
            role: row.role,
            session_token: row.session_token,
            username: row.username,
            first_name: None,
            last_name: None,
            token_version: row.token_version,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

impl From<UserRowNoPassword> for User {
    fn from(row: UserRowNoPassword) -> Self {
        User {
            id: row.id,
            email: row.email,
            password_digest: String::new(),
            role: row.role,
            session_token: row.session_token,
            username: row.username,
            first_name: None,
            last_name: None,
            token_version: row.token_version,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

pub const USER_COLUMNS: &str =
    "id, email, password_digest, role, session_token, username, COALESCE(token_version, 1) as token_version, created_at, updated_at";

pub const USER_COLUMNS_NO_PASSWORD: &str =
    "id, email, role, session_token, username, COALESCE(token_version, 1) as token_version, created_at, updated_at";

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub password_digest: String,
    pub role: Option<String>,
    pub session_token: Option<String>,
    pub username: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub token_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub email: String,
    pub password: String,
    pub username: Option<String>,
    pub token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub username: Option<String>,
    pub role: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserResponse {
    pub id: i64,
    pub email: String,
    pub role: String,
    pub username: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn to_response(&self) -> UserResponse {
        UserResponse {
            id: self.id,
            email: self.email.clone(),
            role: self.role.clone().unwrap_or_else(|| "user".to_string()),
            username: self.username.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn validate_email(email: &str) -> bool {
        if email.is_empty() {
            return false;
        }
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 {
            return false;
        }
        !parts[0].is_empty() && !parts[1].is_empty() && parts[1].contains('.')
    }

    pub fn validate_password(password: &str) -> bool {
        if password.len() < 8 || password.len() > 128 {
            return false;
        }
        let has_upper = password.chars().any(|c| c.is_uppercase());
        let has_lower = password.chars().any(|c| c.is_lowercase());
        let has_digit = password.chars().any(|c| c.is_ascii_digit());
        has_upper && has_lower && has_digit
    }

    pub fn validate_role(role: &str) -> bool {
        matches!(role, "admin" | "super_admin" | "user" | "customer" | "artist" | "staff")
    }

    pub async fn find_by_id(pool: &sqlx::PgPool, id: i64) -> sqlx::Result<Option<Self>> {
        sqlx::query_as::<_, Self>(
            "SELECT id, email, password_digest, role, session_token, username,
                    first_name, last_name, token_version, created_at, updated_at
             FROM users WHERE id = $1",
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
    fn user_to_response_with_role() {
        let user = User {
            id: 1,
            email: "test@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("admin".to_string()),
            session_token: None,
            username: Some("admin".to_string()),
            first_name: None,
            last_name: None,
            token_version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = user.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.email, "test@example.com");
        assert_eq!(resp.role, "admin");
        assert_eq!(resp.username, Some("admin".to_string()));
    }

    #[test]
    fn user_to_response_defaults_role_to_user() {
        let user = User {
            id: 2,
            email: "user@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: None,
            session_token: None,
            username: None,
            first_name: None,
            last_name: None,
            token_version: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = user.to_response();
        assert_eq!(resp.role, "user");
        assert_eq!(resp.username, None);
    }

    #[test]
    fn validate_email_valid() {
        assert!(User::validate_email("test@example.com"));
        assert!(User::validate_email("user+tag@domain.org"));
    }

    #[test]
    fn validate_email_invalid() {
        assert!(!User::validate_email(""));
        assert!(!User::validate_email("no-at-sign"));
        assert!(!User::validate_email("@domain.com"));
    }

    #[test]
    fn validate_password_valid() {
        assert!(User::validate_password("Password1"));
        assert!(User::validate_password("Abcdefg1"));
        assert!(User::validate_password("VeryLongPassword123"));
    }

    #[test]
    fn validate_password_invalid() {
        assert!(!User::validate_password("12345"));
        assert!(!User::validate_password(""));
        assert!(!User::validate_password("password"));
        assert!(!User::validate_password("12345678"));
        assert!(!User::validate_password("PASSWORD1"));
    }

    #[test]
    fn validate_role_valid() {
        assert!(User::validate_role("admin"));
        assert!(User::validate_role("super_admin"));
        assert!(User::validate_role("user"));
        assert!(User::validate_role("customer"));
        assert!(User::validate_role("artist"));
        assert!(User::validate_role("staff"));
    }

    #[test]
    fn validate_role_invalid() {
        assert!(!User::validate_role("moderator"));
        assert!(!User::validate_role(""));
    }

    #[test]
    fn create_user_request_serialization() {
        let req = CreateUserRequest {
            email: "new@example.com".to_string(),
            password: "password123".to_string(),
            username: Some("newuser".to_string()),
            token: Some("trigger-token".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("new@example.com"));
        assert!(json.contains("password123"));
    }

    #[test]
    fn update_user_request_serialization() {
        let req = UpdateUserRequest {
            email: Some("updated@example.com".to_string()),
            username: None,
            role: Some("admin".to_string()),
            password: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("updated@example.com"));
        assert!(json.contains("admin"));
    }
}
