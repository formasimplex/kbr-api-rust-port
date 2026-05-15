//! Authentication handlers
//!
//! Provides endpoints for user login and session management.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `login` | POST | `/v1/auth/login` | public | Authenticate user and return JWT |
//! | `session` | GET | `/v1/auth/session` | auth | Return current user session data |

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::user::User;
use crate::services::auth_service;
use crate::services::user_service::UserService;

#[derive(Debug, FromRow)]
struct UserRow {
    id: i64,
    email: String,
    password_digest: String,
    role: Option<String>,
    session_token: Option<String>,
    username: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
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
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

/// Request body for the login endpoint.
#[derive(Debug, Deserialize)]
pub struct LoginParams {
    pub email: String,
    pub password: String,
}

/// Error response returned on failed login attempts.
#[derive(Debug, Serialize)]
pub struct LoginErrorResponse {
    pub error: String,
}

/// Authenticate a user and return a JWT token.
///
/// Accepts email/username and password. Looks up the user by normalized email
/// or username, verifies the bcrypt password hash, and returns a JWT along
/// with user data on success.
///
/// # Response
///
/// `200 OK` — JSON with `token`, `id`, `email`, `role`, etc.
/// `401 Unauthorized` — `LoginErrorResponse` with `"Invalid credentials"`
pub async fn login(
    state: web::Data<AppState>,
    body: web::Json<LoginParams>,
) -> Result<HttpResponse, AppError> {
    let login_req = auth_service::LoginRequest {
        email: body.email.clone(),
        password: body.password.clone(),
    };

    let normalized_email = UserService::normalize_email(&login_req.email);

    let user_row = sqlx::query_as::<_, UserRow>(
        r"SELECT id, email, password_digest, role, session_token, username, created_at, updated_at
           FROM users WHERE email = $1 OR username = $2"
    )
    .bind(&normalized_email)
    .bind(&normalized_email)
    .fetch_optional(&state.db)
    .await?;

    match user_row {
        Some(row) => {
            let user: User = row.into();
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

/// Return current authenticated user session data.
///
/// Validates the JWT from the Authorization header, looks up the user
/// from the database, and returns fresh session data including a new token.
///
/// # Response
///
/// `200 OK` — JSON with `id`, `email`, `role`, `token`, etc.
/// `401 Unauthorized` — missing or invalid JWT
pub async fn session(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    match sqlx::query_as::<_, UserRow>(
        r"SELECT id, email, password_digest, role, session_token, username, created_at, updated_at
           FROM users WHERE id = $1"
    )
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let u: User = row.into();
            let resp = auth_service::create_session_response(&u)?;
            Ok(HttpResponse::Ok().json(&resp))
        }
        None => Err(AppError::Unauthorized),
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
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";
    const TEST_DB_URL: &str = "postgresql://ws@localhost:5432/kbr_test";

    async fn get_state() -> AppState {
        let pool = sqlx::PgPool::connect(TEST_DB_URL)
            .await
            .expect("Failed to connect to test database");

        let config = crate::services::storage_service::S3Config::from_env()
            .unwrap_or_else(|_| crate::services::storage_service::S3Config {
                access_key: "test".to_string(),
                secret_key: "test".to_string(),
                endpoint: "https://test.test".to_string(),
                bucket_name: "test".to_string(),
                region: "us-east-1".to_string(),
            });
        let s3 = crate::services::storage_service::create_s3_bucket(&config).unwrap_or_else(|_| {
            let creds = s3::creds::Credentials::new(Some("test"), Some("test"), None, None, None).unwrap();
            s3::bucket::Bucket::new("test", s3::region::Region::Custom { region: "us-east-1".to_string(), endpoint: "https://test.test".to_string() }, creds)
                .unwrap()
                .with_path_style()
        });

        AppState { db: pool, s3 }
    }

    async fn seed_test_user(state: &AppState, email: &str, password: &str, role: &str) -> i64 {
        let password_digest = auth_service::hash_password(password).unwrap();
        let now = chrono::Utc::now().naive_utc();
        let row = sqlx::query_as::<_, UserRow>(
            r"INSERT INTO users (email, password_digest, role, session_token, username, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id, email, password_digest, role, session_token, username, created_at, updated_at"
        )
        .bind(email)
        .bind(&password_digest)
        .bind(role)
        .bind::<Option<String>>(None)
        .bind::<Option<String>>(None)
        .bind(&now)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed test user");
        row.id
    }

    async fn cleanup_user(state: &AppState, email: &str) {
        let _ = sqlx::query(r"DELETE FROM users WHERE email = $1")
            .bind(email)
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn login_success_with_valid_credentials() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = get_state().await;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("loginadmin{}@example.com", ts);
        seed_test_user(&state, &email, "correctpassword", "admin").await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/auth/login")
            .set_json(serde_json::json!({
                "email": email,
                "password": "correctpassword"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["token"].is_string());
        assert!(body["id"].as_i64().unwrap_or(0) > 0);

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn login_fails_with_invalid_email() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

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
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = get_state().await;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("loginwrong{}@example.com", ts);
        seed_test_user(&state, &email, "userpassword123", "user").await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/auth/login")
            .set_json(serde_json::json!({
                "email": email,
                "password": "wrongpassword"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["error"], "Invalid credentials");

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn session_returns_fresh_token() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = get_state().await;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("session{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "password123", "admin").await;
        let token = crate::auth::jwt::encode_token_with_role(user_id, TEST_SECRET, 3, Some("admin".to_string())).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/auth/session")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], user_id);
        assert_eq!(body["role"], "admin");
        assert!(body["token"].is_string());

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn session_rejects_without_token() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

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
