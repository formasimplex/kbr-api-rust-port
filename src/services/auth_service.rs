use crate::auth::jwt::{decode_token, encode_token};
use crate::auth::middleware::get_jwt_secret;
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

pub fn create_login_response(user: &User) -> Result<LoginResponse, AppError> {
    let secret = get_jwt_secret()?;
    let token = encode_token(user.id, &secret, JWT_EXPIRY_DAYS)?;
    Ok(LoginResponse {
        token,
        id: user.id,
        role: user.role.clone().unwrap_or_else(|| "user".to_string()),
    })
}

pub fn create_session_response(user: &User) -> Result<SessionResponse, AppError> {
    let secret = get_jwt_secret()?;
    let token = encode_token(user.id, &secret, JWT_EXPIRY_DAYS)?;
    Ok(SessionResponse {
        token,
        id: user.id,
        role: user.role.clone().unwrap_or_else(|| "user".to_string()),
    })
}

pub fn parse_auth_header(header: &str) -> Result<String, AppError> {
    let token = header
        .strip_prefix("Bearer ")
        .or_else(|| {
            if header.is_empty() || !header.contains('.') {
                None
            } else {
                Some(header)
            }
        })
        .ok_or(AppError::Unauthorized)?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err(AppError::Unauthorized);
    }
    Ok(token)
}

pub fn extract_user_id(token: &str) -> Result<i64, AppError> {
    let secret = get_jwt_secret()?;
    let claims = decode_token(token, &secret)?;
    Ok(claims.user_id)
}

pub fn verify_session_token(db_token: &Option<String>, cookie_token: &str) -> bool {
    match db_token {
        Some(t) => t == cookie_token,
        None => false,
    }
}
pub fn generate_session_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| rng.gen_range(b'a'..=b'z'))
        .map(char::from)
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_and_verify_password() {
        let password = "testpassword123";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash).unwrap());
        assert!(!verify_password("wrongpassword", &hash).unwrap());
    }

    #[test]
    fn hash_password_different_each_time() {
        let hash1 = hash_password("samepassword").unwrap();
        let hash2 = hash_password("samepassword").unwrap();
        assert_ne!(hash1, hash2);
        assert!(verify_password("samepassword", &hash1).unwrap());
        assert!(verify_password("samepassword", &hash2).unwrap());
    }

    #[test]
    fn generate_session_token_returns_32_chars() {
        let token = generate_session_token();
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_lowercase()));
    }

    #[test]
    fn generate_session_tokens_are_unique() {
        let token1 = generate_session_token();
        let token2 = generate_session_token();
        assert_ne!(token1, token2);
    }

    #[test]
    fn test_create_login_response() {
        unsafe {
            std::env::set_var("JWT_SECRET", "test-secret-key");
        }

        let user = User {
            id: 42,
            email: "test@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("admin".to_string()),
            session_token: None,
            username: Some("admin".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let resp = create_login_response(&user).unwrap();
        assert_eq!(resp.id, 42);
        assert_eq!(resp.role, "admin");
        assert!(!resp.token.is_empty());
    }

    #[test]
    fn test_create_login_response_defaults_role() {
        unsafe {
            std::env::set_var("JWT_SECRET", "test-secret-key");
        }

        let user = User {
            id: 1,
            email: "test@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: None,
            session_token: None,
            username: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let resp = create_login_response(&user).unwrap();
        assert_eq!(resp.role, "user");
    }

    #[test]
    fn test_create_session_response() {
        unsafe {
            std::env::set_var("JWT_SECRET", "test-secret-key");
        }

        let user = User {
            id: 5,
            email: "test@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("artist".to_string()),
            session_token: None,
            username: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let resp = create_session_response(&user).unwrap();
        assert_eq!(resp.id, 5);
        assert_eq!(resp.role, "artist");
    }

    #[test]
    fn parse_auth_header_with_bearer() {
        let token = parse_auth_header("Bearer abc123token").unwrap();
        assert_eq!(token, "abc123token");
    }

    #[test]
    fn parse_auth_header_raw_token() {
        let token = parse_auth_header("abc123.def456.ghi789").unwrap();
        assert_eq!(token, "abc123.def456.ghi789");
    }

    #[test]
    fn parse_auth_header_empty_returns_error() {
        let result = parse_auth_header("");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_user_id() {
        unsafe {
            std::env::set_var("JWT_SECRET", "test-secret-key");
        }
        let token = encode_token(99, "test-secret-key", 3).unwrap();
        let user_id = extract_user_id(&token).unwrap();
        assert_eq!(user_id, 99);
    }

    #[test]
    fn verify_session_token_matches() {
        let db_token = Some("abc123".to_string());
        assert!(verify_session_token(&db_token, "abc123"));
        assert!(!verify_session_token(&db_token, "xyz789"));
        assert!(!verify_session_token(&None, "abc123"));
    }
}
