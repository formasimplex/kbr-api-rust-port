use crate::error::AppError;
use crate::models::user::{CreateUserRequest, UpdateUserRequest, User};
use crate::services::auth_service::{hash_password, verify_password};

pub struct UserService;

impl UserService {
    pub fn validate_create_request(req: &CreateUserRequest) -> Result<(), AppError> {
        if !User::validate_email(&req.email) {
            return Err(AppError::Validation("Invalid email address".to_string()));
        }
        if !User::validate_password(&req.password) {
            return Err(AppError::Validation(
                "Password must be at least 6 characters".to_string(),
            ));
        }
        if let Some(ref token) = req.token
            && token.is_empty()
        {
            return Err(AppError::Validation(
                "Sign-up token is required".to_string(),
            ));
        }
        Ok(())
    }

    pub fn validate_update_request(req: &UpdateUserRequest) -> Result<(), AppError> {
        if let Some(ref email) = req.email
            && !User::validate_email(email)
        {
            return Err(AppError::Validation("Invalid email address".to_string()));
        }
        if let Some(ref password) = req.password
            && !User::validate_password(password)
        {
            return Err(AppError::Validation(
                "Password must be at least 6 characters".to_string(),
            ));
        }
        if let Some(ref role) = req.role
            && !User::validate_role(role)
        {
            return Err(AppError::Validation(format!("Invalid role: {}", role)));
        }
        Ok(())
    }

    pub fn hash_password_for_create(req: &CreateUserRequest) -> Result<String, AppError> {
        hash_password(&req.password)
    }

    pub fn verify_user_credentials(
        _email: &str,
        password: &str,
        password_digest: &str,
    ) -> Result<bool, AppError> {
        verify_password(password, password_digest)
    }

    pub fn normalize_email(email: &str) -> String {
        email.trim().to_lowercase()
    }

    pub fn force_role_user() -> String {
        "user".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_create_request_valid() {
        let req = CreateUserRequest {
            email: "test@example.com".to_string(),
            password: "Password123".to_string(),
            username: Some("testuser".to_string()),
            token: Some("valid-token".to_string()),
        };
        assert!(UserService::validate_create_request(&req).is_ok());
    }

    #[test]
    fn validate_create_request_invalid_email() {
        let req = CreateUserRequest {
            email: "not-an-email".to_string(),
            password: "Password123".to_string(),
            username: None,
            token: None,
        };
        let err = UserService::validate_create_request(&req).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_create_request_short_password() {
        let req = CreateUserRequest {
            email: "test@example.com".to_string(),
            password: "short".to_string(),
            username: None,
            token: None,
        };
        let err = UserService::validate_create_request(&req).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_create_request_empty_token() {
        let req = CreateUserRequest {
            email: "test@example.com".to_string(),
            password: "Password123".to_string(),
            username: None,
            token: Some("".to_string()),
        };
        let err = UserService::validate_create_request(&req).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_update_request_valid_email() {
        let req = UpdateUserRequest {
            email: Some("new@example.com".to_string()),
            username: None,
            role: None,
            password: None,
        };
        assert!(UserService::validate_update_request(&req).is_ok());
    }

    #[test]
    fn validate_update_request_invalid_email() {
        let req = UpdateUserRequest {
            email: Some("bad-email".to_string()),
            username: None,
            role: None,
            password: None,
        };
        let err = UserService::validate_update_request(&req).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_update_request_short_password() {
        let req = UpdateUserRequest {
            email: None,
            username: None,
            role: None,
            password: Some("short".to_string()),
        };
        let err = UserService::validate_update_request(&req).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_update_request_invalid_role() {
        let req = UpdateUserRequest {
            email: None,
            username: None,
            role: Some("invalid_role".to_string()),
            password: None,
        };
        let err = UserService::validate_update_request(&req).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_update_request_valid_role() {
        let req = UpdateUserRequest {
            email: None,
            username: None,
            role: Some("admin".to_string()),
            password: None,
        };
        assert!(UserService::validate_update_request(&req).is_ok());
    }

    #[test]
    fn normalize_email_lowercase_trim() {
        assert_eq!(
            UserService::normalize_email("  Test@Example.COM  "),
            "test@example.com"
        );
    }
}
