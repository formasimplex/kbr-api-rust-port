//! Reset trigger handlers
//!
//! Provides endpoints for managing password reset triggers. All endpoints
//! are public.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `create` | POST | `/v1/reset_trigger` | public | Create a new password reset trigger for an email address |
//! | `update` | POST | `/v1/reset_trigger/{token}` | public | Reset password using a valid token |

use actix_web::{web, HttpResponse};
use actix_governor::{Governor, GovernorConfigBuilder};
use sqlx::FromRow;
use uuid::Uuid;

use crate::app::AppState;
use crate::error::AppError;
use crate::models::reset_trigger::{ResetTrigger, UpdateResetTriggerRequest};
use crate::models::user::User;
use crate::services::auth_service::hash_password;

async fn reset_jitter() {
    let ms = rand::random::<u64>() % 601 + 200;
    tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
}

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
            email: None,
            user_id: row.user_id,
            full_name: None,
            token: row.token,
            expires_at: row.expires_at,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

/// Create a new password reset trigger for an email address.
///
/// Applies timing jitter before DB access, looks up the user by email,
/// creates a reset trigger with a generated token and expiration time,
/// and enqueues an email job. Returns a generic response regardless of
/// whether the user exists to prevent email enumeration. Public endpoint.
///
/// # Response
///
/// `200 OK` — generic confirmation message
/// `422 Unprocessable` — invalid email
pub async fn create(
    state: web::Data<AppState>,
    body: web::Json<crate::models::reset_trigger::CreateResetTriggerRequest>,
) -> Result<HttpResponse, AppError> {
    if !User::validate_email(&body.email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    tracing::info!(email = %body.email, "Password reset requested");

    reset_jitter().await;

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

    if user_id.is_some() {
        let job_id = Uuid::new_v4();
        if let Err(e) = state
            .job_handle
            .send(crate::jobs::Job::SendResetTriggerEmail {
                job_id,
                reset_trigger_id: trigger.id,
            })
            .await
        {
            tracing::warn!(job_id = %job_id, reset_trigger_id = trigger.id, error = %e, "Failed to enqueue reset trigger email job");
        }
    }

    Ok(HttpResponse::Ok().json(trigger.to_response()))
}

/// Reset password using a valid token.
///
/// Validates the token within a database transaction (SELECT FOR UPDATE),
/// checks password strength (minimum 8 characters, uppercase, lowercase,
/// digit), confirms password matches confirmation, verifies the new password
/// differs from the current one, hashes the new password, updates the user
/// record, increments token_version to invalidate existing sessions, and
/// deletes the reset trigger atomically. Public endpoint.
///
/// # Response
///
/// `200 OK` — success confirmation
/// `400 Bad Request` — reset token has expired
/// `404 Not Found` — token does not exist or has no associated user
/// `422 Unprocessable` — password validation failure
pub async fn update(
    state: web::Data<AppState>,
    path: web::Path<String>,
    body: web::Json<UpdateResetTriggerRequest>,
) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();

    if !User::validate_password(&body.password) {
        return Err(AppError::Validation(
            "Password must be at least 8 characters with uppercase, lowercase, and a digit"
                .to_string(),
        ));
    }

    if body.password != body.password_confirmation {
        return Err(AppError::Validation(
            "Passwords do not match".to_string(),
        ));
    }

    let mut tx = state.db.begin().await?;

    let trigger = sqlx::query_as::<_, ResetTriggerRow>(
        r"SELECT id, user_id, token, expires_at, created_at, updated_at
           FROM reset_triggers WHERE token = $1 FOR UPDATE"
    )
    .bind(&token)
    .fetch_optional(&mut *tx)
    .await?;

    let trigger = match trigger {
        Some(t) => t,
        None => {
            return Err(AppError::NotFound("Reset trigger not found".to_string()));
        }
    };

    if ResetTrigger::is_expired(trigger.expires_at.as_deref()) {
        return Err(AppError::BadRequest("Reset token has expired".to_string()));
    }

    let user_id = trigger.user_id.ok_or_else(|| {
        AppError::NotFound("Reset trigger has no associated user".to_string())
    })?;

    let current_hash: String = sqlx::query_scalar(
        "SELECT password_digest FROM users WHERE id = $1"
    )
    .bind(user_id as i64)
    .fetch_one(&mut *tx)
    .await
    .map_err(|_| AppError::NotFound("User not found".to_string()))?;

    if bcrypt::verify(&body.password, &current_hash).map_err(AppError::Bcrypt)? {
        return Err(AppError::Validation(
            "New password must be different from your current password".to_string(),
        ));
    }

    let new_hash = hash_password(&body.password)?;
    let now = chrono::Utc::now().naive_utc();

    sqlx::query(
        r"UPDATE users SET password_digest = $1, token_version = COALESCE(token_version, 1) + 1, updated_at = $2 WHERE id = $3"
    )
    .bind(&new_hash)
    .bind(now)
    .bind(user_id as i64)
    .execute(&mut *tx)
    .await?;

    sqlx::query(r"DELETE FROM reset_triggers WHERE token = $1")
        .bind(&token)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    tracing::info!(user_id = user_id as i64, "Password reset completed");

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Password updated successfully"
    })))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    let reset_create_governor = GovernorConfigBuilder::default()
        .seconds_per_request(5)
        .burst_size(3)
        .finish()
        .expect("valid governor config");

    let reset_update_governor = GovernorConfigBuilder::default()
        .seconds_per_request(3)
        .burst_size(5)
        .finish()
        .expect("valid governor config");

    cfg.route("/reset_trigger", web::post()
        .wrap(Governor::new(&reset_create_governor))
        .to(create))
        .route("/reset_trigger/{token}", web::post()
        .wrap(Governor::new(&reset_update_governor))
        .to(update));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::get_test_state;
    use actix_web::{test, App};

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_create_returns_generic_response() {
        crate::test_utils::set_test_env();
        let (state, app) = crate::build_test_app!(config_routes);

        let email = format!("reset_create_{}@example.com", chrono::Utc::now().timestamp_micros());

        let req = test::TestRequest::post()
            .uri("/reset_trigger")
            .set_json(serde_json::json!({
                "email": email
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["message"].is_string());
        assert!(!body["message"].as_str().unwrap().contains("token"));

        let _ = sqlx::query(r"DELETE FROM reset_triggers WHERE user_id IS NOT NULL")
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_create_nonexistent_email_returns_same_response() {
        crate::test_utils::set_test_env();
        let (state, app) = crate::build_test_app!(config_routes);

        let nonexistent = format!("nonexistent_{}@example.com", chrono::Utc::now().timestamp_micros());

        let req = test::TestRequest::post()
            .uri("/reset_trigger")
            .set_json(serde_json::json!({
                "email": nonexistent
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["message"].is_string());

        let _ = sqlx::query(r"DELETE FROM reset_triggers WHERE user_id IS NULL")
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_create_invalid_email() {
        crate::test_utils::set_test_env();
        let (_state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/reset_trigger")
            .set_json(serde_json::json!({
                "email": "bad-email"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_update_success() {
        crate::test_utils::set_test_env();
        let state = web::Data::new(get_test_state().await);

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
               VALUES ($1, $2, '2027-12-31T23:59:00Z', NOW(), NOW())"
        )
        .bind(user_id as i32)
        .bind(&token)
        .execute(&state.db)
        .await
        .expect("Failed to create reset trigger");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/reset_trigger/{}", token))
            .set_json(serde_json::json!({
                "password": "Newpassword123",
                "password_confirmation": "Newpassword123"
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
    async fn reset_trigger_update_password_mismatch() {
        crate::test_utils::set_test_env();
        let state = web::Data::new(get_test_state().await);

        let email = format!("reset_mm_{}@example.com", chrono::Utc::now().timestamp_micros());
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

        let token = format!("rust_reset_mm_{}", chrono::Utc::now().timestamp_micros());
        sqlx::query(
            r"INSERT INTO reset_triggers (user_id, token, expires_at, created_at, updated_at)
               VALUES ($1, $2, '2027-12-31T23:59:00Z', NOW(), NOW())"
        )
        .bind(user_id as i32)
        .bind(&token)
        .execute(&state.db)
        .await
        .expect("Failed to create reset trigger");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/reset_trigger/{}", token))
            .set_json(serde_json::json!({
                "password": "Newpassword123",
                "password_confirmation": "Differentpass1"
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
        assert_eq!(old_hash, original_hash, "Password should NOT have changed");

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
        crate::test_utils::set_test_env();
        let state = web::Data::new(get_test_state().await);

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
               VALUES ($1, $2, '2027-12-31T23:59:00Z', NOW(), NOW())"
        )
        .bind(user_id as i32)
        .bind(&token)
        .execute(&state.db)
        .await
        .expect("Failed to create reset trigger");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/reset_trigger/{}", token))
            .set_json(serde_json::json!({
                "password": "short",
                "password_confirmation": "short"
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
    async fn reset_trigger_update_same_password_rejected() {
        crate::test_utils::set_test_env();
        let state = web::Data::new(get_test_state().await);

        let email = format!("reset_same_{}@example.com", chrono::Utc::now().timestamp_micros());
        let current_password = "Currentpass1";
        let old_hash = hash_password(current_password).unwrap();
        let user_id: i64 = sqlx::query_scalar(
            r"INSERT INTO users (email, password_digest, created_at, updated_at)
               VALUES ($1, $2, NOW(), NOW()) RETURNING id"
        )
        .bind(&email)
        .bind(&old_hash)
        .fetch_one(&state.db)
        .await
        .expect("Failed to create test user");

        let token = format!("rust_reset_same_{}", chrono::Utc::now().timestamp_micros());
        sqlx::query(
            r"INSERT INTO reset_triggers (user_id, token, expires_at, created_at, updated_at)
               VALUES ($1, $2, '2027-12-31T23:59:00Z', NOW(), NOW())"
        )
        .bind(user_id as i32)
        .bind(&token)
        .execute(&state.db)
        .await
        .expect("Failed to create reset trigger");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/reset_trigger/{}", token))
            .set_json(serde_json::json!({
                "password": current_password,
                "password_confirmation": current_password
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        let stored_hash: String = sqlx::query_scalar(
            r"SELECT password_digest FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .expect("User not found");
        assert_eq!(old_hash, stored_hash, "Password should NOT have changed");

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
        crate::test_utils::set_test_env();
        let state = web::Data::new(get_test_state().await);

        let token = format!("rust_reset_nu_{}", chrono::Utc::now().timestamp_micros());
        sqlx::query(
            r"INSERT INTO reset_triggers (user_id, token, expires_at, created_at, updated_at)
               VALUES (NULL, $1, '2027-12-31T23:59:00Z', NOW(), NOW())"
        )
        .bind(&token)
        .execute(&state.db)
        .await
        .expect("Failed to create reset trigger");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/reset_trigger/{}", token))
            .set_json(serde_json::json!({
                "password": "Newpassword123",
                "password_confirmation": "Newpassword123"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        let _ = sqlx::query(r"DELETE FROM reset_triggers WHERE token = $1")
            .bind(&token)
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_update_expired_token_rejected() {
        crate::test_utils::set_test_env();
        let state = get_test_state().await;

        let email = format!("reset_exp_{}@example.com", chrono::Utc::now().timestamp_micros());
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

        let past = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let token = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            r"INSERT INTO reset_triggers (user_id, token, expires_at, created_at, updated_at)
               VALUES ($1, $2, $3, NOW(), NOW())"
        )
        .bind(user_id as i32)
        .bind(&token)
        .bind(&past)
        .execute(&state.db)
        .await
        .expect("Failed to create reset trigger");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/reset_trigger/{}", token))
            .set_json(serde_json::json!({
                "password": "Newpassword123",
                "password_confirmation": "Newpassword123"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 400);

        let new_hash: String = sqlx::query_scalar(
            r"SELECT password_digest FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .expect("User not found");
        assert_eq!(old_hash, new_hash, "Password should NOT have changed for expired token");

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
    async fn reset_trigger_update_token_consumed_once() {
        crate::test_utils::set_test_env();
        let state = web::Data::new(get_test_state().await);

        let email = format!("reset_once_{}@example.com", chrono::Utc::now().timestamp_micros());
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

        let token = format!("rust_reset_once_{}", chrono::Utc::now().timestamp_micros());
        sqlx::query(
            r"INSERT INTO reset_triggers (user_id, token, expires_at, created_at, updated_at)
               VALUES ($1, $2, '2027-12-31T23:59:00Z', NOW(), NOW())"
        )
        .bind(user_id as i32)
        .bind(&token)
        .execute(&state.db)
        .await
        .expect("Failed to create reset trigger");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .configure(config_routes),
        )
        .await;

        let req1 = test::TestRequest::post()
            .uri(&format!("/reset_trigger/{}", token))
            .set_json(serde_json::json!({
                "password": "Newpassword123",
                "password_confirmation": "Newpassword123"
            }))
            .to_request();
        let resp1 = test::call_service(&app, req1).await;
        assert_eq!(resp1.status(), 200);

        let req2 = test::TestRequest::post()
            .uri(&format!("/reset_trigger/{}", token))
            .set_json(serde_json::json!({
                "password": "Anotherpass123",
                "password_confirmation": "Anotherpass123"
            }))
            .to_request();
        let resp2 = test::call_service(&app, req2).await;
        assert_eq!(resp2.status(), 404);

        let _ = sqlx::query(r"DELETE FROM reset_triggers WHERE token = $1")
            .bind(&token)
            .execute(&state.db)
            .await;
        let _ = sqlx::query(r"DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&state.db)
            .await;
    }
}
