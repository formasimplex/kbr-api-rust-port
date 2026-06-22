//! Sign-up trigger handlers
//!
//! Provides endpoints for managing sign-up triggers. Endpoints are
//! public for use during user registration flows.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `create` | POST | `/v1/sign_up_trigger` | public | Create a new sign-up trigger for an email address |
//! | `show` | GET | `/v1/sign_up_trigger/{token}` | public | Retrieve an existing sign-up trigger by token |

use actix_web::{web, HttpResponse};
use sqlx::FromRow;
use uuid::Uuid;

use crate::app::AppState;
use crate::error::AppError;
use crate::models::sign_up_trigger::SignUpTrigger;
use crate::models::user::User;
use crate::services::user_service::UserService;

#[derive(Debug, FromRow)]
struct SignUpTriggerRow {
    id: i64,
    email: Option<String>,
    token: Option<String>,
    expires_at: Option<String>,
    role: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<SignUpTriggerRow> for SignUpTrigger {
    fn from(row: SignUpTriggerRow) -> Self {
        SignUpTrigger {
            id: row.id,
            email: row.email,
            full_name: None,
            confirmation_token: None,
            token: row.token,
            expires_at: row.expires_at,
            role: row.role,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

/// Create a new sign-up trigger for an email address.
///
/// Checks for artist conflict, expires any existing trigger for the same
/// email, generates a new token with 1-day expiry, and sends a role-specific
/// confirmation email. Public endpoint.
///
/// # Response
///
/// `201 Created` — `SignUpTriggerResponse` with the new trigger
/// `403 Forbidden` — email already linked to an artist account
/// `422 Unprocessable` — invalid email
pub async fn create(
    state: web::Data<AppState>,
    body: web::Json<crate::models::sign_up_trigger::CreateSignUpTriggerRequest>,
) -> Result<HttpResponse, AppError> {
    let normalized_email = UserService::normalize_email(&body.email);

    if !User::validate_email(&normalized_email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    let is_artist: bool = sqlx::query_scalar(
        r"SELECT EXISTS(
            SELECT 1 FROM users u
            JOIN artists a ON a.user_id = u.id
            WHERE u.email = $1 AND u.role = 'artist'
        )"
    )
    .bind(&normalized_email)
    .fetch_one(&state.db)
    .await
    .unwrap_or(false);

    if is_artist {
        return Err(AppError::Forbidden(
            "Email already linked to Artist account".to_string(),
        ));
    }

    let now = chrono::Utc::now().naive_utc();
    let now_str = chrono::Utc::now().to_rfc3339();

    let _ = sqlx::query(
        r"UPDATE sign_up_triggers SET expires_at = $1, updated_at = $2 WHERE email = $3"
    )
    .bind(&now_str)
    .bind(now)
    .bind(&normalized_email)
    .execute(&state.db)
    .await;

    let token = SignUpTrigger::generate_token();
    let expires_at = SignUpTrigger::generate_expires_at();

    let row = sqlx::query_as::<_, SignUpTriggerRow>(
        r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, email, token, expires_at, role, created_at, updated_at"
    )
    .bind(&normalized_email)
    .bind(&token)
    .bind(&expires_at)
    .bind(&body.role)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let trigger: SignUpTrigger = row.into();

    let job_id = Uuid::new_v4();
    if let Err(e) = state
        .job_handle
        .send(crate::jobs::Job::SendSignUpTriggerEmail {
            job_id,
            sign_up_trigger_id: trigger.id,
        })
        .await
    {
        tracing::warn!(job_id = %job_id, sign_up_trigger_id = trigger.id, error = %e, "Failed to enqueue sign-up trigger email job");
    }

    Ok(HttpResponse::Created().json(trigger.to_response()))
}

/// Retrieve an existing sign-up trigger by token.
///
/// Public endpoint. Returns the sign-up trigger details if the
/// token exists.
///
/// # Response
///
/// `200 OK` — `SignUpTriggerResponse`
/// `404 Not Found` — token does not exist
pub async fn show(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();

    match sqlx::query_as::<_, SignUpTriggerRow>(
        r"SELECT id, email, token, expires_at, role, created_at, updated_at
           FROM sign_up_triggers WHERE token = $1"
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let trigger: SignUpTrigger = row.into();
            if trigger.is_expired() {
                return Err(AppError::NotFound("Sign-up trigger has expired".to_string()));
            }
            Ok(HttpResponse::Ok().json(trigger.to_response()))
        }
        None => Err(AppError::NotFound("Sign-up trigger not found".to_string())),
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/sign_up_trigger", web::post().to(create))
        .route("/sign_up_trigger/{token}", web::get().to(show));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::get_test_state;
    use actix_web::test;

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_create_success() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/sign_up_trigger")
            .set_json(serde_json::json!({
                "email": "invited@example.com",
                "role": "user"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["email"], "invited@example.com");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_create_invalid_email() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/sign_up_trigger")
            .set_json(serde_json::json!({
                "email": "bad-email"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_create_artist_conflict_returns_403() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let ts = chrono::Utc::now().timestamp_micros();
        let email = format!("artist_conflict_{}@example.com", ts);
        let password_digest = "hashed_password_test".to_string();
        let now = chrono::Utc::now().naive_utc();

        let user_id: i64 = sqlx::query_scalar(
            r"INSERT INTO users (email, password_digest, role, created_at, updated_at)
               VALUES ($1, $2, 'artist', $3, $3) RETURNING id"
        )
        .bind(&email)
        .bind(&password_digest)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed artist user");

        let _artist_id: i64 = sqlx::query_scalar(
            r"INSERT INTO artists (name, user_id, created_at, updated_at)
               VALUES ($1, $2, $3, $3) RETURNING id"
        )
        .bind(format!("Test Artist {}", ts))
        .bind(user_id)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed artist");

        let req = test::TestRequest::post()
            .uri("/sign_up_trigger")
            .set_json(serde_json::json!({
                "email": email,
                "role": "user"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_create_expires_existing_trigger() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = chrono::Utc::now().timestamp_micros();
        let email = format!("existing_trigger_{}@example.com", ts);

        let old_token = format!("old_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        let now = chrono::Utc::now().naive_utc();
        let old_id: i64 = sqlx::query_scalar(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ($1, $2, $3, 'user', $4, $4) RETURNING id"
        )
        .bind(&email)
        .bind(&old_token)
        .bind(&future)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed existing trigger");

        let req = test::TestRequest::post()
            .uri("/sign_up_trigger")
            .set_json(serde_json::json!({
                "email": email,
                "role": "user"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let old_expires: Option<String> = sqlx::query_scalar(
            r"SELECT expires_at FROM sign_up_triggers WHERE id = $1"
        )
        .bind(old_id)
        .fetch_one(&state.db)
        .await
        .expect("Failed to check old trigger");

        assert!(
            SignUpTrigger::parse_expires_at_for_test(Some(old_expires.as_deref().unwrap()))
                .unwrap()
                < chrono::Utc::now(),
            "Old trigger should be expired"
        );

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_show_valid() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let token = format!("rust_test_{}", chrono::Utc::now().timestamp_micros());
    let seed = sqlx::query_as::<_, SignUpTriggerRow>(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ('showtest@example.com', $1, '2027-12-31T23:59:00+00:00', 'user', NOW(), NOW())
               RETURNING id, email, token, expires_at, role, created_at, updated_at"
        )
        .bind(&token)
        .fetch_one(&state.db)
        .await;

        if let Ok(_row) = seed {
            let req = test::TestRequest::get()
                .uri(&format!("/sign_up_trigger/{}", token))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);

            let body: serde_json::Value = test::read_body_json(resp).await;
            assert_eq!(body["email"], "showtest@example.com");
        }

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_show_not_found() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/sign_up_trigger/nonexistent-token-xyz")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_show_expired_returns_404() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let token = format!("expired_test_{}", chrono::Utc::now().timestamp_micros());
        let past = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();
        let _ = sqlx::query_as::<_, SignUpTriggerRow>(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ('expired@example.com', $1, $2, 'user', NOW(), NOW())
               RETURNING id, email, token, expires_at, role, created_at, updated_at"
        )
        .bind(&token)
        .bind(&past)
        .fetch_one(&state.db)
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/sign_up_trigger/{}", token))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }
}
