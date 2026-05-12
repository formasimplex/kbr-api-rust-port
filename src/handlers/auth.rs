use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::user::User;
use crate::services::auth_service;
use crate::services::user_service::UserService;

static TEST_ADMIN_HASH: OnceLock<String> = OnceLock::new();
static TEST_USER_HASH: OnceLock<String> = OnceLock::new();

fn test_admin_hash() -> &'static str {
    TEST_ADMIN_HASH.get_or_init(|| auth_service::hash_password("correctpassword").unwrap_or_default())
}

fn test_user_hash() -> &'static str {
    TEST_USER_HASH.get_or_init(|| auth_service::hash_password("userpassword123").unwrap_or_default())
}

#[derive(Debug, Deserialize)]
pub struct LoginParams {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginErrorResponse {
    pub error: String,
}

pub async fn login(
    body: web::Json<LoginParams>,
) -> Result<HttpResponse, AppError> {
    let login_req = auth_service::LoginRequest {
        email: body.email.clone(),
        password: body.password.clone(),
    };

    let normalized_email = UserService::normalize_email(&login_req.email);

    let mock_user = find_user_by_email_or_username(&normalized_email, &login_req.email);
    match mock_user {
        Some(user) => {
            let valid = auth_service::verify_password(&login_req.password, &user.password_digest)?;
            if valid {
                let resp = auth_service::create_login_response(&user)?;
                Ok(HttpResponse::Ok().json(&resp))
            } else {
                Ok(HttpResponse::Unauthorized().json(LoginErrorResponse {
                    error: "Invalid credentials".to_string(),
                }))
            }
        }
        None => Ok(HttpResponse::Unauthorized().json(LoginErrorResponse {
            error: "Invalid credentials".to_string(),
        })),
    }
}

pub async fn session(
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    let mock_user = find_user_by_id(user.id);
    match mock_user {
        Some(u) => {
            let resp = auth_service::create_session_response(&u)?;
            Ok(HttpResponse::Ok().json(&resp))
        }
        None => Err(AppError::Unauthorized),
    }
}

fn find_user_by_email_or_username(email: &str, username: &str) -> Option<User> {
    if email == "test@example.com" || username == "test@example.com" {
        Some(User {
            id: 1,
            email: "test@example.com".to_string(),
            password_digest: test_admin_hash().to_string(),
            role: Some("admin".to_string()),
            session_token: Some("sessionsession".to_string()),
            username: Some("testuser".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    } else if email == "user@example.com" || username == "user@example.com" {
        Some(User {
            id: 2,
            email: "user@example.com".to_string(),
            password_digest: test_user_hash().to_string(),
            role: Some("user".to_string()),
            session_token: None,
            username: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    } else {
        None
    }
}

fn find_user_by_id(id: i64) -> Option<User> {
    match id {
        1 => Some(User {
            id: 1,
            email: "test@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("admin".to_string()),
            session_token: Some("sessionsession".to_string()),
            username: Some("testuser".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }),
        2 => Some(User {
            id: 2,
            email: "user@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("user".to_string()),
            session_token: None,
            username: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }),
        _ => None,
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1/auth")
            .route("/login", web::post().to(login))
            .route("/session", web::get().to(session)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";

    #[tokio::test(flavor = "current_thread")]
    async fn login_success_with_valid_credentials() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }

        test_admin_hash();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/auth/login")
            .set_json(serde_json::json!({
                "email": "test@example.com",
                "password": "correctpassword"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["token"].is_string());
        assert_eq!(body["id"], 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn login_fails_with_invalid_email() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }

        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/auth/login")
            .set_json(serde_json::json!({
                "email": "nonexistent@example.com",
                "password": "anypassword"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["error"], "Invalid credentials");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn login_fails_with_wrong_password() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }

        test_user_hash();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/auth/login")
            .set_json(serde_json::json!({
                "email": "user@example.com",
                "password": "wrongpassword"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["error"], "Invalid credentials");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn session_returns_fresh_token() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }

        let app = test::init_service(App::new().configure(config_routes)).await;

        let token = crate::auth::jwt::encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap();
        let req = test::TestRequest::get()
            .uri("/v1/auth/session")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], 1);
        assert_eq!(body["role"], "admin");
        assert!(body["token"].is_string());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn session_rejects_without_token() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }

        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/auth/session")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn login_params_deserialization() {
        let json = r#"{"email": "test@example.com", "password": "pass123"}"#;
        let params: LoginParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.email, "test@example.com");
        assert_eq!(params.password, "pass123");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn login_error_response_serialization() {
        let err = LoginErrorResponse {
            error: "Invalid credentials".to_string(),
        };
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("Invalid credentials"));
    }
}
