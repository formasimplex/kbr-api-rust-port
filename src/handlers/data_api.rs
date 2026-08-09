//! Data API handlers
//!
//! Provides internal data-access endpoints for analytics and GDPR
//! compliance. All endpoints require authentication.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `last_logins` | GET | `/v1/data/last_logins` | auth | Get last login info for all users |
//! | `last_login_by_id` | GET | `/v1/data/last_logins/{user_id}` | auth | Get last login info for a specific user |
//! | `event_attendees_present` | POST | `/v1/data/event_attendees_present` | auth | Build a set of present attendees for a date range |

use actix_web::{web, HttpResponse};
use serde::Serialize;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::data::data_api as data;
use crate::error::AppError;

/// Response for a user's last login information.
#[derive(Debug, Serialize)]
struct LastLoginResponse {
    username: Option<String>,
    last_login: String,
}

/// Response for a specific user's last login information.
#[derive(Debug, Serialize)]
struct LastLoginByIdResponse {
    email: String,
    last_login: String,
}

/// Get last login information for the 10 most recently updated users.
///
/// Returns username and last login timestamp in RFC3339 format.
/// Requires authentication.
///
/// # Response
///
/// `200 OK` — JSON array of `LastLoginResponse`
pub async fn last_logins(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("must be admin".into()));
    }
    let rows = data::last_logins(&state.db).await?;

    let responses: Vec<LastLoginResponse> = rows
        .into_iter()
        .map(|r| LastLoginResponse {
            username: r.username,
            last_login: r.updated_at.and_utc().to_rfc3339(),
        })
        .collect();

    Ok(HttpResponse::Ok().json(responses))
}

/// Get last login information for a specific user by ID.
///
/// Returns email and last login timestamp in RFC3339 format.
/// Requires authentication.
///
/// # Response
///
/// `200 OK` — `LastLoginByIdResponse`
/// `404 Not Found` — user does not exist
pub async fn last_login_by_id(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("must be admin".into()));
    }
    let id = path.into_inner();

    match data::last_login_by_id(&state.db, id).await? {
        Some(row) => Ok(HttpResponse::Ok().json(LastLoginByIdResponse {
            email: row.email,
            last_login: row.updated_at.and_utc().to_rfc3339(),
        })),
        None => Err(AppError::NotFound(format!("User #{}", id))),
    }
}

/// Build a set of present attendees for an event.
///
/// Returns distinct mail subscribers who have a positive scan count
/// for the given event. Requires authentication.
///
/// # Response
///
/// `200 OK` — JSON array of `EventAttendeePresentResponse`
/// `404 Not Found` — event does not exist
pub async fn event_attendees_present(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("must be admin".into()));
    }
    let event_id = path.into_inner();

    if !data::event_exists(&state.db, event_id).await? {
        return Err(AppError::NotFound(format!("Event #{}", event_id)));
    }

    let rows = data::event_attendees_present(&state.db, event_id as i32).await?;

    Ok(HttpResponse::Ok().json(rows))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/data/last_logins", web::get().to(last_logins))
        .route("/data/last_logins/{id}", web::get().to(last_login_by_id))
        .route("/data/event_attendees_present/{id}", web::get().to(event_attendees_present));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{admin_token, not_found_id, seed_attendee, seed_event, seed_mail_subscriber, unique_suffix};
    use actix_web::test;

    async fn seed_user_with_username(pool: &sqlx::PgPool) -> (i64, String) {
        let email = format!("dataapi_user_{}@test.com", unique_suffix());
        let username = format!("dataapi_user_{}", unique_suffix());
        let id: i64 = sqlx::query_scalar(
            r#"INSERT INTO users (email, password_digest, username, role, created_at, updated_at)
               VALUES ($1, $2, $3, $4, NOW(), NOW())
               RETURNING id"#
        )
        .bind(&email)
        .bind("hashed_password_test".to_string())
        .bind(Some(username))
        .bind(Some("user".to_string()))
        .fetch_one(pool)
        .await
        .expect("Failed to seed user");
        (id, email)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn data_last_logins_returns_users() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/data/last_logins")
            .insert_header((
                actix_web::http::header::AUTHORIZATION,
                format!("Bearer {}", admin_token()),
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert!(body.len() <= 10);
        for item in &body {
            assert!(item.get("username").is_some());
            assert!(item.get("last_login").is_some());
        }

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn data_last_login_by_id_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, user_email) = seed_user_with_username(&state.db).await;

        let req = test::TestRequest::get()
            .uri(&format!("/data/last_logins/{}", user_id))
            .insert_header((
                actix_web::http::header::AUTHORIZATION,
                format!("Bearer {}", admin_token()),
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["email"], user_email);
        assert!(body.get("last_login").is_some());

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn data_last_login_by_id_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "users").await;

        let req = test::TestRequest::get()
            .uri(&format!("/data/last_logins/{}", not_found))
            .insert_header((
                actix_web::http::header::AUTHORIZATION,
                format!("Bearer {}", admin_token()),
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn data_event_attendees_present_returns_scanned() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let event_id = seed_event(&state.db, None).await;
        let sub1 = seed_mail_subscriber(&state.db, None, None).await;
        let sub2 = seed_mail_subscriber(&state.db, None, None).await;
        let sub3 = seed_mail_subscriber(&state.db, None, None).await;

        seed_attendee(&state.db, event_id, sub1, 2).await;
        seed_attendee(&state.db, event_id, sub2, 1).await;
        seed_attendee(&state.db, event_id, sub3, 0).await;

        let req = test::TestRequest::get()
            .uri(&format!("/data/event_attendees_present/{}", event_id))
            .insert_header((
                actix_web::http::header::AUTHORIZATION,
                format!("Bearer {}", admin_token()),
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 2);
        for item in &body {
            assert!(item.get("email").is_some());
            assert!(item.get("full_name").is_some());
            assert!(item.get("scan_count").is_some());
            assert!(item["scan_count"].as_i64().unwrap() > 0);
        }

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn data_event_attendees_present_event_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "kbr_events").await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/data/event_attendees_present/{}",
                not_found
            ))
            .insert_header((
                actix_web::http::header::AUTHORIZATION,
                format!("Bearer {}", admin_token()),
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }
}
