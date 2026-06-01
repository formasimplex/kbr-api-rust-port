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
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;

#[derive(Debug, FromRow)]
struct LastLoginRow {
    username: Option<String>,
    updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, FromRow)]
struct LastLoginByIdRow {
    email: String,
    updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, FromRow, Serialize)]
struct EventAttendeePresentRow {
    id: i64,
    scan_count: Option<i32>,
    email: String,
    full_name: String,
}

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
    let rows = sqlx::query_as::<_, LastLoginRow>(
        r#"SELECT username, updated_at FROM users ORDER BY updated_at DESC LIMIT 10"#
    )
    .fetch_all(&state.db)
    .await?;

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

    match sqlx::query_as::<_, LastLoginByIdRow>(
        r#"SELECT email, updated_at FROM users WHERE id = $1"#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
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

    let event_exists = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM kbr_events WHERE id = $1"#
    )
    .bind(event_id)
    .fetch_one(&state.db)
    .await?;

    if event_exists == 0 {
        return Err(AppError::NotFound(format!("Event #{}", event_id)));
    }

    let rows = sqlx::query_as::<_, EventAttendeePresentRow>(
        r#"
        SELECT DISTINCT ON (ma.mail_subscriber_id)
            ma.id,
            ma.scan_count,
            ms.email,
            ms.full_name
        FROM kbr_event_attendees ma
        JOIN mail_subscribers ms ON ms.id = ma.mail_subscriber_id
        WHERE ma.kbr_event_id = $1 AND ma.scan_count > 0
        ORDER BY ma.mail_subscriber_id, ma.id
        "#
    )
    .bind(event_id as i32)
    .fetch_all(&state.db)
    .await?;

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
    use crate::auth::jwt::encode_token_with_role;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";

    use std::sync::atomic::{AtomicI64, Ordering};
    static TEST_COUNTER: AtomicI64 = AtomicI64::new(0);

    fn unique_suffix() -> String {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{}_{}", ts, count)
    }

    fn admin_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap()
    }

async fn get_state() -> AppState {
        let pool = sqlx::PgPool::connect(crate::test_utils::test_db_url())
            .await
            .expect("Failed to connect to test database");
        crate::test_utils::build_test_state(pool).await
    }

    async fn seed_user() -> (i64, String) {
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
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed user");
        (id, email)
    }

    async fn cleanup_user(_user_id: i64, email: &str) {
        let _ = sqlx::query(r#"DELETE FROM users WHERE email = $1"#)
            .bind(email)
            .execute(&get_state().await.db)
            .await;
    }

    async fn seed_event() -> i64 {
        let now = chrono::Utc::now().naive_utc();
        sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO kbr_events (name, description, active, event_start_date, event_end_date, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id"#
        )
        .bind(format!("DataApi Event {}", unique_suffix()))
        .bind("A test event for data api")
        .bind(true)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .bind(&now)
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed event")
    }

    async fn cleanup_event(event_id: i64) {
        let _ = sqlx::query(r#"DELETE FROM kbr_event_attendees WHERE kbr_event_id = $1"#)
            .bind(event_id as i32)
            .execute(&get_state().await.db)
            .await;
        let _ = sqlx::query(r#"DELETE FROM kbr_events WHERE id = $1"#)
            .bind(event_id)
            .execute(&get_state().await.db)
            .await;
    }

    async fn seed_mail_subscriber() -> i64 {
        let email = format!("dataapi_sub_{}@test.com", unique_suffix());
        sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO mail_subscribers (full_name, email, created_at, updated_at)
               VALUES ($1, $2, NOW(), NOW())
               RETURNING id"#
        )
        .bind(format!("Subscriber {}", unique_suffix()))
        .bind(&email)
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed mail subscriber")
    }

    async fn seed_attendee(event_id: i64, subscriber_id: i64, scan_count: i32) -> i64 {
        sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO kbr_event_attendees (kbr_event_id, mail_subscriber_id, scan_count, created_at, updated_at)
               VALUES ($1, $2, $3, NOW(), NOW())
               RETURNING id"#
        )
        .bind(event_id as i32)
        .bind(subscriber_id as i32)
        .bind(scan_count)
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed attendee")
    }

    #[tokio::test(flavor = "current_thread")]
    async fn data_last_logins_returns_users() {
        unsafe { std::env::set_var("DATABASE_URL", crate::test_utils::test_db_url()); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

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
    }

    #[tokio::test(flavor = "current_thread")]
    async fn data_last_login_by_id_found() {
        unsafe { std::env::set_var("DATABASE_URL", crate::test_utils::test_db_url()); }
        let state = web::Data::new(get_state().await);

        let (user_id, user_email) = seed_user().await;

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

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

        cleanup_user(user_id, &user_email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn data_last_login_by_id_not_found() {
        unsafe { std::env::set_var("DATABASE_URL", crate::test_utils::test_db_url()); }
        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(r#"SELECT COALESCE(MAX(id), 0) FROM users"#)
            .fetch_one(&state.db)
            .await
            .expect("Failed to get max id");

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/data/last_logins/{}", max_id + 9999))
            .insert_header((
                actix_web::http::header::AUTHORIZATION,
                format!("Bearer {}", admin_token()),
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn data_event_attendees_present_returns_scanned() {
        unsafe { std::env::set_var("DATABASE_URL", crate::test_utils::test_db_url()); }
        let state = web::Data::new(get_state().await);

        let event_id = seed_event().await;
        let sub1 = seed_mail_subscriber().await;
        let sub2 = seed_mail_subscriber().await;
        let sub3 = seed_mail_subscriber().await;

        seed_attendee(event_id, sub1, 2).await;
        seed_attendee(event_id, sub2, 1).await;
        seed_attendee(event_id, sub3, 0).await;

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

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

        cleanup_event(event_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn data_event_attendees_present_event_not_found() {
        unsafe { std::env::set_var("DATABASE_URL", crate::test_utils::test_db_url()); }
        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(r#"SELECT COALESCE(MAX(id), 0) FROM kbr_events"#)
            .fetch_one(&state.db)
            .await
            .expect("Failed to get max id");

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!(
                "/data/event_attendees_present/{}",
                max_id + 9999
            ))
            .insert_header((
                actix_web::http::header::AUTHORIZATION,
                format!("Bearer {}", admin_token()),
            ))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }
}
