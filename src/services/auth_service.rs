use uuid::Uuid;

use crate::auth::jwt::{decode_token, encode_token_with_role, Claims};
use crate::error::AppError;
use crate::models::user::User;

const JWT_EXPIRY_DAYS: u64 = 3;

pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(serde::Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub id: i64,
    pub role: String,
}

#[derive(serde::Serialize)]
pub struct SessionResponse {
    pub token: String,
    pub id: i64,
    pub role: String,
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(AppError::Bcrypt)?;
    Ok(hash)
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let verified = bcrypt::verify(password, hash).map_err(AppError::Bcrypt)?;
    Ok(verified)
}

pub fn create_login_response(user: &User, secret: &str) -> Result<LoginResponse, AppError> {
    let token = encode_token_with_role(
        user.id,
        secret,
        JWT_EXPIRY_DAYS,
        user.role.clone(),
        user.token_version,
    )?;
    Ok(LoginResponse {
        token,
        id: user.id,
        role: user.role.clone().unwrap_or_else(|| "user".to_string()),
    })
}

pub fn create_session_response(user: &User, secret: &str) -> Result<SessionResponse, AppError> {
    let token = encode_token_with_role(
        user.id,
        secret,
        JWT_EXPIRY_DAYS,
        user.role.clone(),
        user.token_version,
    )?;
    Ok(SessionResponse {
        token,
        id: user.id,
        role: user.role.clone().unwrap_or_else(|| "user".to_string()),
    })
}

pub fn create_claims(user: &User, secret: &str) -> Result<Claims, AppError> {
    let token = encode_token_with_role(
        user.id,
        secret,
        JWT_EXPIRY_DAYS,
        user.role.clone(),
        user.token_version,
    )?;
    decode_token(&token, secret)
}

pub fn parse_auth_header(header: &str) -> Result<String, AppError> {
    let token = header
        .strip_prefix("Bearer ")
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .ok_or(AppError::Unauthorized)?;
    Ok(token)
}

pub fn extract_user_id(token: &str, secret: &str) -> Result<i64, AppError> {
    let claims = decode_token(token, secret)?;
    Ok(claims.user_id)
}

pub fn verify_session_token(db_token: &Option<String>, cookie_token: &str) -> bool {
    match db_token {
        Some(t) => t == cookie_token,
        None => false,
    }
}

pub fn generate_session_token() -> String {
    Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-key";

    #[test]
    fn generate_session_token_returns_uuid() {
        let token = generate_session_token();
        let uuid: Uuid = token.parse().expect("should be valid UUID");
        assert_eq!(uuid.get_version(), Some(uuid::Version::Random));
    }

    #[test]
    fn generate_session_tokens_are_unique() {
        let token1 = generate_session_token();
        let token2 = generate_session_token();
        assert_ne!(token1, token2);
    }

    #[test]
 fn test_create_login_response() {
        let user = User {
            id: 42,
            email: "test@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("admin".to_string()),
            session_token: None,
            username: Some("admin".to_string()),
            first_name: None,
            last_name: None,
            token_version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let resp = create_login_response(&user, TEST_SECRET).unwrap();
        assert_eq!(resp.id, 42);
        assert_eq!(resp.role, "admin");
        assert!(!resp.token.is_empty());
    }

    #[test]
    fn test_create_login_response_defaults_role() {
        let user = User {
            id: 1,
            email: "test@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: None,
            session_token: None,
            username: None,
            first_name: None,
            last_name: None,
            token_version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let resp = create_login_response(&user, TEST_SECRET).unwrap();
        assert_eq!(resp.role, "user");
    }

    #[test]
    fn test_create_session_response() {
        let user = User {
            id: 5,
            email: "test@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("artist".to_string()),
            session_token: None,
            username: None,
            first_name: None,
            last_name: None,
            token_version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let resp = create_session_response(&user, TEST_SECRET).unwrap();
        assert_eq!(resp.id, 5);
        assert_eq!(resp.role, "artist");
    }

    #[test]
    fn parse_auth_header_with_bearer() {
        let token = parse_auth_header("Bearer abc123token").unwrap();
        assert_eq!(token, "abc123token");
    }

    #[test]
    fn parse_auth_header_raw_token_rejected() {
        let result = parse_auth_header("abc123.def456.ghi789");
        assert!(result.is_err());
    }

    #[test]
    fn parse_auth_header_empty_returns_error() {
        let result = parse_auth_header("");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_user_id() {
        let token = crate::auth::jwt::encode_token(99, TEST_SECRET, 3).unwrap();
        let user_id = extract_user_id(&token, TEST_SECRET).unwrap();
        assert_eq!(user_id, 99);
    }

    #[test]
    fn verify_session_token_matches() {
        let db_token = Some("abc123".to_string());
        assert!(verify_session_token(&db_token, "abc123"));
        assert!(!verify_session_token(&db_token, "xyz789"));
        assert!(!verify_session_token(&None, "abc123"));
    }

    #[test]
    fn test_create_claims() {
        let user = User {
            id: 42,
            email: "test@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("admin".to_string()),
            session_token: None,
            username: Some("admin".to_string()),
            first_name: None,
            last_name: None,
            token_version: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let claims = create_claims(&user, TEST_SECRET).unwrap();
        assert_eq!(claims.user_id, 42);
        assert!(claims.iat > 0);
        assert!(!claims.jti.is_empty());
    }
}
