use crate::data::sign_up_trigger as data_sign_up_trigger;
use crate::data::users as data_users;
use crate::error::AppError;
use crate::models::sign_up_trigger::SignUpTrigger;
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
                "Password must be at least 8 characters with uppercase, lowercase, and a digit".to_string(),
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
                "Password must be at least 8 characters with uppercase, lowercase, and a digit".to_string(),
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

    pub async fn create_user_with_trigger(
        pool: &sqlx::PgPool,
        req: &CreateUserRequest,
    ) -> Result<User, AppError> {
        let normalized_email = Self::normalize_email(&req.email);
        let token = req.token.as_deref().unwrap();

        let mut tx = pool.begin().await?;

        let trigger = data_sign_up_trigger::by_token_for_update(&mut tx, token).await?;

        match trigger {
            Some((trigger_email, trigger_expires)) => {
                if let Some(expires_str) = trigger_expires
                    && let Some(expired_time) = SignUpTrigger::parse_expires_at(&expires_str)
                        && expired_time < chrono::Utc::now()
                {
                    return Err(AppError::Validation(
                        "Sign-up token has expired".to_string(),
                    ));
                }

                let emails_match = trigger_email
                    .as_ref()
                    .map(|e| Self::normalize_email(e))
                    .as_deref()
                    == Some(&normalized_email);

                if !emails_match {
                    return Err(AppError::Validation(
                        "Email does not match sign-up token".to_string(),
                    ));
                }
            }
            None => {
                return Err(AppError::Validation("Invalid sign-up token".to_string()));
            }
        }

        let email_exists = data_users::exists_by_email_in_tx(&mut tx, &normalized_email).await?;

        if email_exists {
            return Err(AppError::Conflict(
                "A user with this email already exists".to_string(),
            ));
        }

        let password_digest = Self::hash_password_for_create(req)?;
        let role = Self::force_role_user();
        let now = chrono::Utc::now().naive_utc();

        let user = data_users::create_in_tx(
            &mut tx,
            &normalized_email,
            &password_digest,
            &role,
            &req.username,
            now,
        )
        .await?;

        let now_str = chrono::Utc::now().to_rfc3339();
        let _ = sqlx::query(
            r"UPDATE sign_up_triggers SET expires_at = $1, updated_at = $2 WHERE email = $3",
        )
        .bind(&now_str)
        .bind(now)
        .bind(&normalized_email)
        .execute(&mut *tx)
        .await;

        tx.commit().await?;

        Ok(user)
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
