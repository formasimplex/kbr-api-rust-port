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
use rand::Rng;
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
    token_version: i64,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, FromRow)]
struct UserRowNoPassword {
    id: i64,
    email: String,
    role: Option<String>,
    session_token: Option<String>,
    username: Option<String>,
    token_version: i64,
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

async fn login_jitter() {
    let ms = rand::thread_rng().gen_range(200..=800);
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

fn is_production() -> bool {
    !std::env::var("CORS_ORIGINS").unwrap_or_default().trim().is_empty()
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

    // Apply timing jitter on ALL code paths to prevent user enumeration via timing
    login_jitter().await;

    let normalized_email = UserService::normalize_email(&login_req.email);

    let user_row = sqlx::query_as::<_, UserRow>(
        r"SELECT id, email, password_digest, role, session_token, username, COALESCE(token_version, 1) as token_version, created_at, updated_at
           FROM users WHERE email = $1"
    )
    .bind(&normalized_email)
    .fetch_optional(&state.db)
    .await?;

    match user_row {
        Some(row) => {
            let user: User = row.into();
            let valid = auth_service::verify_password(&login_req.password, &user.password_digest)?;
            if valid {
                let resp = auth_service::create_login_response(&user, &state.jwt_secret)?;
                tracing::info!(
                    user_id = user.id,
                    event = "login_success",
                );
                Ok(HttpResponse::Ok().json(&resp))
            } else {
                tracing::warn!(
                    email = %normalized_email,
                    event = "login_failed",
                    reason = "invalid_password",
                );
                Ok(HttpResponse::Unauthorized().json(LoginErrorResponse {
                    error: "Invalid credentials".to_string(),
                }))
            }
        }
        None => {
            tracing::warn!(
                email = %normalized_email,
                event = "login_failed",
                reason = "user_not_found",
            );
            Ok(HttpResponse::Unauthorized().json(LoginErrorResponse {
                error: "Invalid credentials".to_string(),
            }))
        }
    }
}

/// Return current authenticated user session data.
///
/// Validates the JWT from the Authorization header or encrypted cookie,
/// verifies the role against the database, checks token revocation,
/// and returns fresh session data including a new token and encrypted cookie.
///
/// # Response
///
/// `200 OK` — JSON with `id`, `email`, `role`, `token`, etc.
/// `401 Unauthorized` — missing or invalid JWT
 pub async fn session(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    let db_role = user.verify_role_with_db(&state.db).await?;

    let user_row = sqlx::query_as::<_, UserRowNoPassword>(
        r"SELECT id, email, role, session_token, username, COALESCE(token_version, 1) as token_version, created_at, updated_at
           FROM users WHERE id = $1"
    )
    .bind(user.id)
    .fetch_optional(&state.db)
    .await?;

    match user_row {
        Some(row) => {
            let u: User = row.into();
            let resp = auth_service::create_session_response(&u, &state.jwt_secret)?;
            let claims = auth_service::create_claims(&u, &state.jwt_secret)?;

            let cookie_builder = &state.cookie_builder;
            let cookie = cookie_builder.create(claims);
            let cookie_finished = if is_production() {
                cookie
                    .http_only(true)
                    .secure(true)
                    .same_site(actix_web::cookie::SameSite::Lax)
                    .finish()
            } else {
                cookie.http_only(true).finish()
            };

            tracing::info!(
                user_id = u.id,
                event = "session_refresh",
                db_role = %db_role,
            );

            Ok(HttpResponse::Ok()
                .cookie(cookie_finished)
                .json(&resp))
        }
        None => {
            tracing::warn!(
                user_id = user.id,
                event = "session_rejected",
                reason = "user_not_found",
            );
            Err(AppError::Unauthorized)
        }
    }
}

/// Revoke the current user's JWT token.
///
/// Inserts the token's `jti` claim into the `revoked_tokens` table so that
/// subsequent requests with the same token are rejected. The token is
/// extracted from the `CurrentUser` by the middleware before this handler runs.
///
/// # Response
///
/// `200 OK` — JSON with `"message": "logged out successfully"`
/// `401 Unauthorized` — missing or invalid JWT
pub async fn logout(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    user.revoke_token(&state.db).await?;
    user.invalidate_all_sessions(&state.db).await?;

    tracing::info!(
        user_id = user.id,
        event = "logout_success",
    );

    let cookie_name = state.cookie_builder.name.as_ref();
    let clear_cookie = actix_web::cookie::Cookie::build(cookie_name, "")
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(actix_web::cookie::SameSite::Lax)
        .max_age(actix_web::cookie::time::Duration::seconds(0))
        .finish();

    Ok(HttpResponse::Ok()
        .cookie(clear_cookie)
        .json(serde_json::json!({
            "message": "logged out successfully"
        })))
}

  pub fn config_routes(cfg: &mut web::ServiceConfig) {
    let login_governor = actix_governor::GovernorConfigBuilder::default()
        .seconds_per_request(2)
        .burst_size(5)
        .finish()
        .expect("valid governor config");

    cfg.route("/login", web::post()
        .wrap(actix_governor::Governor::new(&login_governor))
        .to(login));
    cfg.route("/session", web::get().to(session));
    cfg.route("/logout", web::post().to(logout));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::TEST_SECRET;
    use actix_web::{test, App};

    async fn seed_test_user(state: &AppState, email: &str, password: &str, role: &str) -> i64 {
        let password_digest = auth_service::hash_password(password).unwrap();
        let now = chrono::Utc::now().naive_utc();
        let row = sqlx::query_as::<_, UserRow>(
            r"INSERT INTO users (email, password_digest, role, session_token, username, token_version, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, 1, $6, $7)
               RETURNING id, email, password_digest, role, session_token, username, token_version, created_at, updated_at"
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
        crate::test_utils::set_test_env_jwt();
        let state = get_test_state().await;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("loginadmin{}@example.com", ts);
        seed_test_user(&state, &email, "correctpassword", "admin").await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .app_data(web::Data::new(
                    get_test_state().await.cookie_builder.clone()
                ))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/login")
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
        crate::test_utils::set_test_env_jwt();
        let state = web::Data::new(get_test_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/login")
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
        crate::test_utils::set_test_env_jwt();
        let state = get_test_state().await;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("loginwrong{}@example.com", ts);
        seed_test_user(&state, &email, "userpassword123", "user").await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/login")
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
        crate::test_utils::set_test_env_jwt();
        let state = get_test_state().await;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("session{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "password123", "admin").await;
        let token = crate::auth::jwt::encode_token_with_role(user_id, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .app_data(web::Data::new(
                    get_test_state().await.cookie_builder.clone()
                ))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/session")
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
        crate::test_utils::set_test_env_jwt();
        let state = web::Data::new(get_test_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/session")
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

    #[tokio::test(flavor = "current_thread")]
    async fn logout_success() {
        crate::test_utils::set_test_env_jwt();
        let state = get_test_state().await;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("logout{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "password123", "user").await;
        let token = crate::auth::jwt::encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .app_data(web::Data::new(
                    get_test_state().await.cookie_builder.clone()
                ))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/logout")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["message"], "logged out successfully");

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn logout_rejects_without_token() {
        crate::test_utils::set_test_env_jwt();
        let state = web::Data::new(get_test_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/logout")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn logout_prevents_further_session_requests() {
        crate::test_utils::set_test_env_jwt();
        let state = get_test_state().await;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("logoutsession{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "password123", "user").await;
        let token = crate::auth::jwt::encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .app_data(web::Data::new(
                    get_test_state().await.cookie_builder.clone()
                ))
                .configure(config_routes),
        )
        .await;

        // Logout first
        let req = test::TestRequest::post()
            .uri("/logout")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // Same token should now be rejected
        let req = test::TestRequest::get()
            .uri("/session")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn logout_idempotent() {
        crate::test_utils::set_test_env_jwt();
        let state = get_test_state().await;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("logoutidempotent{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "password123", "user").await;
        let token = crate::auth::jwt::encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .app_data(web::Data::new(
                    get_test_state().await.cookie_builder.clone()
                ))
                .configure(config_routes),
        )
        .await;

        // First logout
        let req = test::TestRequest::post()
            .uri("/logout")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // Second logout with same token - middleware will reject since token is revoked
        // This is expected behavior: once revoked, the token can't be used again
        let req = test::TestRequest::post()
            .uri("/logout")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn logout_invalidates_all_sessions_for_user() {
        crate::test_utils::set_test_env_jwt();
        let state = get_test_state().await;
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("logoutmulti{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "password123", "user").await;
        let token1 = crate::auth::jwt::encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();
        let token2 = crate::auth::jwt::encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .app_data(web::Data::new(
                    get_test_state().await.cookie_builder.clone()
                ))
                .configure(config_routes),
        )
        .await;

        // Logout with token1
        let req = test::TestRequest::post()
            .uri("/logout")
            .insert_header(("Authorization", format!("Bearer {}", token1)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // token2 should also be rejected due to token_version increment
        let req = test::TestRequest::get()
            .uri("/session")
            .insert_header(("Authorization", format!("Bearer {}", token2)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);

        cleanup_user(&state, &email).await;
    }
}
