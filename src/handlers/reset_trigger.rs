//! Reset trigger handlers
//!
//! Provides endpoints for managing password reset triggers. All endpoints
//! require authentication.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `create` | POST | `/v1/reset_trigger` | public | Create a new password reset trigger for an email address |
//! | `show` | GET | `/v1/reset_trigger/{token}` | public | Retrieve an existing reset trigger by token |
//! | `update` | POST | `/v1/reset_trigger/{token}` | public | Reset password using a valid token |

use actix_web::{web, HttpResponse};
use serde::Deserialize;
use sqlx::FromRow;

use crate::app::AppState;
use crate::error::AppError;
use crate::models::reset_trigger::{ResetTrigger, UpdateResetTriggerRequest};
use crate::models::user::User;
use crate::services::auth_service::hash_password;

#[derive(Debug, FromRow)]
struct ResetTriggerRow {
    id: i64,
    user_id: Option<i32>,
    token: Option<String>,
    expires_at: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<ResetTriggerRow> for ResetTrigger {
    fn from(row: ResetTriggerRow) -> Self {
        ResetTrigger {
            id: row.id,
            user_id: row.user_id,
            token: row.token,
            expires_at: row.expires_at,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

/// Create a new password reset trigger for an email address.
///
/// Looks up the user by email and creates a reset trigger with a
/// generated token and expiration time. Public endpoint.
///
/// # Response
///
/// `201 Created` — `ResetTriggerResponse` with the new trigger
/// `422 Unprocessable` — invalid email
pub async fn create(
    state: web::Data<AppState>,
    body: web::Json<crate::models::reset_trigger::CreateResetTriggerRequest>,
) -> Result<HttpResponse, AppError> {
    if !User::validate_email(&body.email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    let user_id: Option<i64> = sqlx::query_scalar(
        r"SELECT id FROM users WHERE email = $1"
    )
    .bind(&body.email)
    .fetch_optional(&state.db)
    .await?;

    let user_id_int: Option<i32> = user_id.map(|id| id as i32);

    let token = ResetTrigger::generate_token();
    let expires_at = ResetTrigger::generate_expires_at();
    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, ResetTriggerRow>(
        r"INSERT INTO reset_triggers (user_id, token, expires_at, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, user_id, token, expires_at, created_at, updated_at"
    )
    .bind(user_id_int)
    .bind(&token)
    .bind(&expires_at)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let trigger: ResetTrigger = row.into();
    Ok(HttpResponse::Created().json(trigger.to_response()))
}

/// Retrieve an existing reset trigger by token.
///
/// Public endpoint. Returns the reset trigger details if the token
/// exists.
///
/// # Response
///
/// `200 OK` — `ResetTriggerResponse`
/// `404 Not Found` — token does not exist
pub async fn show(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();

    match sqlx::query_as::<_, ResetTriggerRow>(
        r"SELECT id, user_id, token, expires_at, created_at, updated_at
           FROM reset_triggers WHERE token = $1"
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let trigger: ResetTrigger = row.into();
            Ok(HttpResponse::Ok().json(trigger.to_response()))
        }
        None => Err(AppError::NotFound("Reset trigger not found".to_string())),
    }
}

/// Reset password using a valid token.
///
/// Validates the token, checks password length (minimum 6 characters),
/// hashes the new password, updates the user record, and deletes the
/// reset trigger. Public endpoint.
///
/// # Response
///
/// `200 OK` — success confirmation
/// `404 Not Found` — token does not exist or has no associated user
/// `422 Unprocessable` — password too short
pub async fn update(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateResetTriggerRequest>,
) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();

    match sqlx::query_as::<_, ResetTriggerRow>(
        r"SELECT id, user_id, token, expires_at, created_at, updated_at
           FROM reset_triggers WHERE token = $1"
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await?
    {
        Some(trigger) => {
            let user_id = trigger.user_id.ok_or_else(|| {
                AppError::NotFound("Reset trigger has no associated user".to_string())
            })?;

            if !User::validate_password(&body.password) {
                return Err(AppError::Validation(
                    "Password must be at least 6 characters".to_string(),
                ));
            }

            let new_hash = hash_password(&body.password)?;
            let now = chrono::Utc::now().naive_utc();

            sqlx::query(
                r"UPDATE users SET password_digest = $1, updated_at = $2 WHERE id = $3"
            )
            .bind(&new_hash)
            .bind(now)
            .bind(user_id as i64)
            .execute(&state.db)
            .await?;

            sqlx::query(r"DELETE FROM reset_triggers WHERE token = $1")
                .bind(&token)
                .execute(&state.db)
                .await?;

            Ok(HttpResponse::Ok().json(serde_json::json!({
                "message": "Password updated successfully"
            })))
        }
        None => Err(AppError::NotFound("Reset trigger not found".to_string())),
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/reset_trigger", web::post().to(create))
            .route("/reset_trigger/{token}", web::get().to(show))
            .route("/reset_trigger/{token}", web::post().to(update)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

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

        AppState { db: pool, s3, shopify: None, mailchimp: None, safe_browsing: None }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_create_success() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let email = format!("reset_create_{}@example.com", chrono::Utc::now().timestamp_micros());
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/reset_trigger")
            .set_json(serde_json::json!({
                "email": email
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["token"].is_string());

        if let Some(token) = body["token"].as_str() {
            let _ = sqlx::query(r"DELETE FROM reset_triggers WHERE token = $1")
                .bind(token)
                .execute(&state.db)
                .await;
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_create_invalid_email() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/reset_trigger")
            .set_json(serde_json::json!({
                "email": "bad-email"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_show_valid() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let token = format!("rust_reset_{}", chrono::Utc::now().timestamp_micros());
        let seed = sqlx::query_as::<_, ResetTriggerRow>(
            r"INSERT INTO reset_triggers (user_id, token, expires_at, created_at, updated_at)
               VALUES (NULL, $1, 'Dec 31, 2026 11:59 PM', NOW(), NOW())
               RETURNING id, user_id, token, expires_at, created_at, updated_at"
        )
        .bind(&token)
        .fetch_one(&state.db)
        .await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        if let Ok(_row) = seed {
            let req = test::TestRequest::get()
                .uri(&format!("/v1/reset_trigger/{}", token))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);

            let _ = sqlx::query(r"DELETE FROM reset_triggers WHERE token = $1")
                .bind(&token)
                .execute(&state.db)
                .await;
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_show_not_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/reset_trigger/bad-token-nonexistent")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_update_success() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let email = format!("reset_upd_{}@example.com", chrono::Utc::now().timestamp_micros());
        let old_hash = hash_password("oldpassword").unwrap();
        let user_id: i64 = sqlx::query_scalar(
            r"INSERT INTO users (email, password_digest, created_at, updated_at)
               VALUES ($1, $2, NOW(), NOW()) RETURNING id"
        )
        .bind(&email)
        .bind(&old_hash)
        .fetch_one(&state.db)
        .await
        .expect("Failed to create test user");

        let token = format!("rust_reset_up_{}", chrono::Utc::now().timestamp_micros());
        sqlx::query(
            r"INSERT INTO reset_triggers (user_id, token, expires_at, created_at, updated_at)
               VALUES ($1, $2, 'Dec 31, 2026 11:59 PM', NOW(), NOW())"
        )
        .bind(user_id as i32)
        .bind(&token)
        .execute(&state.db)
        .await
        .expect("Failed to create reset trigger");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/v1/reset_trigger/{}", token))
            .set_json(serde_json::json!({
                "password": "newpassword123"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let new_hash: String = sqlx::query_scalar(
            r"SELECT password_digest FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .expect("User not found");
        assert_ne!(old_hash, new_hash, "Password should have been updated");

        let _ = sqlx::query(r"DELETE FROM reset_triggers WHERE token = $1")
            .bind(&token)
            .execute(&state.db)
            .await;
        let _ = sqlx::query(r"DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_update_short_password() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let email = format!("reset_sp_{}@example.com", chrono::Utc::now().timestamp_micros());
        let old_hash = hash_password("oldpassword").unwrap();
        let user_id: i64 = sqlx::query_scalar(
            r"INSERT INTO users (email, password_digest, created_at, updated_at)
               VALUES ($1, $2, NOW(), NOW()) RETURNING id"
        )
        .bind(&email)
        .bind(&old_hash)
        .fetch_one(&state.db)
        .await
        .expect("Failed to create test user");

        let token = format!("rust_reset_sp_{}", chrono::Utc::now().timestamp_micros());
        sqlx::query(
            r"INSERT INTO reset_triggers (user_id, token, expires_at, created_at, updated_at)
               VALUES ($1, $2, 'Dec 31, 2026 11:59 PM', NOW(), NOW())"
        )
        .bind(user_id as i32)
        .bind(&token)
        .execute(&state.db)
        .await
        .expect("Failed to create reset trigger");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/v1/reset_trigger/{}", token))
            .set_json(serde_json::json!({
                "password": "short"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        let original_hash: String = sqlx::query_scalar(
            r"SELECT password_digest FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .expect("User not found");
        assert_eq!(old_hash, original_hash, "Password should NOT have changed for invalid input");

        let _ = sqlx::query(r"DELETE FROM reset_triggers WHERE token = $1")
            .bind(&token)
            .execute(&state.db)
            .await;
        let _ = sqlx::query(r"DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_update_no_user() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let token = format!("rust_reset_nu_{}", chrono::Utc::now().timestamp_micros());
        sqlx::query(
            r"INSERT INTO reset_triggers (user_id, token, expires_at, created_at, updated_at)
               VALUES (NULL, $1, 'Dec 31, 2026 11:59 PM', NOW(), NOW())"
        )
        .bind(&token)
        .execute(&state.db)
        .await
        .expect("Failed to create reset trigger");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/v1/reset_trigger/{}", token))
            .set_json(serde_json::json!({
                "password": "newpassword123"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        let _ = sqlx::query(r"DELETE FROM reset_triggers WHERE token = $1")
            .bind(&token)
            .execute(&state.db)
            .await;
    }
}
