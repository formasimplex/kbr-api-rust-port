//! Event attendee handlers
//!
//! Provides endpoints for managing event attendees, including QR code
//! scanning, listing attendees, and creating/updating attendee records.
//! All endpoints require artist role or above.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `qr_scan` | GET | `/v1/qr_scan/{id}` | public | Scan a QR code to increment attendee scan count |
//! | `attendees_for_event` | GET | `/v1/kbr_event_attendees` | artist+ | List all attendees for a specific event by kbr_event_id query param |
//! | `create` | POST | `/v1/kbr_event_attendees` | artist+ | Create new event attendee records from mail subscriber IDs |
//! | `update` | POST | `/v1/kbr_event_update_txt` | artist+ | Return attendees for an event (used for text copy updates) |

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_artist_or_above;
use crate::error::AppError;
use crate::models::kbr_event_attendee::{
    CreateEventAttendeeRequest, KbrEventAttendee, KbrEventAttendeeResponse,
    UpdateEventAttendeeRequest,
};

#[derive(Debug, FromRow)]
struct EventAttendeeRow {
    id: i64,
    kbr_event_id: Option<i32>,
    mail_subscriber_id: Option<i32>,
    scan_count: Option<i32>,
    headcount: Option<i32>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<EventAttendeeRow> for KbrEventAttendee {
    fn from(row: EventAttendeeRow) -> Self {
        KbrEventAttendee {
            id: row.id,
            kbr_event_id: row.kbr_event_id,
            mail_subscriber_id: row.mail_subscriber_id,
            scan_count: row.scan_count,
            headcount: row.headcount,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

/// Scan a QR code to increment attendee scan count.
///
/// Increments the scan_count by 1 for the given attendee ID.
/// No authentication required.
///
/// # Response
///
/// `200 OK` — `KbrEventAttendeeResponse` with updated scan count
/// `404 Not Found` — attendee does not exist
pub async fn qr_scan(
    path: web::Path<i64>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    let row = sqlx::query_as::<_, EventAttendeeRow>(
        r"UPDATE kbr_event_attendees
           SET scan_count = COALESCE(scan_count, 0) + 1, updated_at = NOW()
           WHERE id = $1
           RETURNING id, kbr_event_id, mail_subscriber_id, scan_count, headcount, created_at, updated_at"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(r) => {
            let attendee: KbrEventAttendee = r.into();
            Ok(HttpResponse::Ok().json(attendee.to_response()))
        }
        None => Err(AppError::NotFound(format!("Attendee #{}", id))),
    }
}

/// List all attendees for a specific event.
///
/// Requires artist role or above. The kbr_event_id is passed as a
/// query parameter.
///
/// # Response
///
/// `200 OK` — JSON array of `KbrEventAttendeeResponse`
/// `403 Forbidden` — insufficient role
/// `400 Bad Request` — missing or invalid kbr_event_id
pub async fn attendees_for_event(
    user: CurrentUser,
    query: web::Query<serde_json::Value>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let event_id = match query.0.get("kbr_event_id") {
        Some(serde_json::Value::Number(n)) => n.as_i64().ok_or_else(|| {
            AppError::BadRequest("Invalid kbr_event_id".to_string())
        })?,
        Some(serde_json::Value::String(s)) => s.parse::<i64>().map_err(|_| {
            AppError::BadRequest("Invalid kbr_event_id".to_string())
        })?,
        _ => return Err(AppError::BadRequest("kbr_event_id is required".to_string())),
    };

    let rows = sqlx::query_as::<_, EventAttendeeRow>(
        r"SELECT id, kbr_event_id, mail_subscriber_id, scan_count, headcount, created_at, updated_at
           FROM kbr_event_attendees WHERE kbr_event_id = $1"
    )
    .bind(event_id as i32)
    .fetch_all(&state.db)
    .await?;

    let attendees: Vec<KbrEventAttendee> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<KbrEventAttendeeResponse> =
        attendees.iter().map(|a| a.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Create new event attendee records from mail subscriber IDs.
///
/// Requires artist role or above. Creates one attendee record per
/// mail_subscriber_id in a transaction.
///
/// # Response
///
/// `201 Created` — JSON array of `KbrEventAttendeeResponse`
/// `403 Forbidden` — insufficient role
/// `422 Unprocessable` — empty mail_subscriber_ids
pub async fn create(
    user: CurrentUser,
    body: web::Json<CreateEventAttendeeRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    if body.mail_subscriber_ids.is_empty() {
        return Err(AppError::Validation(
            "mail_subscriber_ids is required".to_string(),
        ));
    }

    let mut tx = state.db.begin().await?;
    let mut attendees = Vec::new();

    for subscriber_id in &body.mail_subscriber_ids {
        let now = chrono::Utc::now().naive_utc();
        let row = sqlx::query_as::<_, EventAttendeeRow>(
            r"INSERT INTO kbr_event_attendees (kbr_event_id, mail_subscriber_id, scan_count, headcount, created_at, updated_at)
               VALUES ($1, $2, 0, $3, $4, $4)
               RETURNING id, kbr_event_id, mail_subscriber_id, scan_count, headcount, created_at, updated_at"
        )
        .bind(body.kbr_event_id)
        .bind(*subscriber_id)
        .bind(body.headcount)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        attendees.push(KbrEventAttendee::from(row));
    }

    tx.commit().await?;

    let responses: Vec<KbrEventAttendeeResponse> =
        attendees.iter().map(|a| a.to_response()).collect();
    Ok(HttpResponse::Created().json(responses))
}

/// Return attendees for an event.
///
/// Requires artist role or above. Used for text copy updates —
/// returns all attendees matching the given kbr_event_id.
///
/// # Response
///
/// `200 OK` — JSON array of `KbrEventAttendeeResponse`
/// `403 Forbidden` — insufficient role
pub async fn update(
    user: CurrentUser,
    body: web::Json<UpdateEventAttendeeRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    let rows = sqlx::query_as::<_, EventAttendeeRow>(
        r"SELECT id, kbr_event_id, mail_subscriber_id, scan_count, headcount, created_at, updated_at
           FROM kbr_event_attendees WHERE kbr_event_id = $1"
    )
    .bind(body.kbr_event_id)
    .fetch_all(&state.db)
    .await?;

    let attendees: Vec<KbrEventAttendee> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<KbrEventAttendeeResponse> =
        attendees.iter().map(|a| a.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/qr_scan/{id}", web::get().to(qr_scan))
            .route("/kbr_event_attendees", web::get().to(attendees_for_event))
            .route("/kbr_event_attendees", web::post().to(create))
            .route("/kbr_event_update_txt", web::post().to(update)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token_with_role;
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

        AppState { db: pool, s3, shopify: None, mailchimp: None, safe_browsing: None }
    }

    fn admin_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    fn suffix() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }

    async fn seed_event(state: &AppState) -> i64 {
        let name = format!("Test Event {}", suffix());
        let id: i64 = sqlx::query_scalar(
            r"INSERT INTO kbr_events (name, created_at, updated_at) VALUES ($1, NOW(), NOW()) RETURNING id"
        )
        .bind(&name)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed event");
        id
    }

    async fn seed_mail_subscriber(state: &AppState, name: &str) -> i64 {
        let email = format!("{}@test.com", name);
        let id: i64 = sqlx::query_scalar(
            r"INSERT INTO mail_subscribers (full_name, email, created_at, updated_at) VALUES ($1, $2, NOW(), NOW()) RETURNING id"
        )
        .bind(name)
        .bind(&email)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed mail subscriber");
        id
    }

    async fn seed_attendee(state: &AppState, event_id: i64, subscriber_id: i64) -> i64 {
        let id: i64 = sqlx::query_scalar(
            r"INSERT INTO kbr_event_attendees (kbr_event_id, mail_subscriber_id, scan_count, created_at, updated_at)
               VALUES ($1, $2, 0, NOW(), NOW()) RETURNING id"
        )
        .bind(event_id)
        .bind(subscriber_id)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed attendee");
        id
    }

    async fn cleanup(state: &AppState, event_id: i64, subscriber_ids: &[i64]) {
        let _ = sqlx::query(r"DELETE FROM kbr_event_attendees WHERE kbr_event_id = $1")
            .bind(event_id as i32)
            .execute(&state.db)
            .await;

        for sid in subscriber_ids {
            let _ = sqlx::query(r"DELETE FROM mail_subscribers WHERE id = $1")
                .bind(*sid)
                .execute(&state.db)
                .await;
        }

        let _ = sqlx::query(r"DELETE FROM kbr_events WHERE id = $1")
            .bind(event_id)
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn qr_scan_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let event_id = seed_event(&state).await;
        let sub_id = seed_mail_subscriber(&state, &format!("QRScanTest {}", suffix())).await;
        let attendee_id = seed_attendee(&state, event_id, sub_id).await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/qr_scan/{}", attendee_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["scan_count"], 1);

        cleanup(&state, event_id, &[sub_id]).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn qr_scan_not_found() {
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
            .uri("/v1/qr_scan/99999999")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn attendees_for_event_authenticated() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let event_id = seed_event(&state).await;
        let sub1 = seed_mail_subscriber(&state, &format!("EventSub1 {}", suffix())).await;
        let sub2 = seed_mail_subscriber(&state, &format!("EventSub2 {}", suffix())).await;
        seed_attendee(&state, event_id, sub1).await;
        seed_attendee(&state, event_id, sub2).await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/kbr_event_attendees?kbr_event_id={}", event_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 2);

        cleanup(&state, event_id, &[sub1, sub2]).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn attendees_for_event_forbidden() {
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
            .uri("/v1/kbr_event_attendees?kbr_event_id=1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_client_error());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn create_attendee_authenticated() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let event_id = seed_event(&state).await;
        let sub1 = seed_mail_subscriber(&state, &format!("CreateSub1 {}", suffix())).await;
        let sub2 = seed_mail_subscriber(&state, &format!("CreateSub2 {}", suffix())).await;

        let req = test::TestRequest::post()
            .uri("/v1/kbr_event_attendees")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "kbr_event_id": event_id as i32,
                "mail_subscriber_ids": [sub1 as i32, sub2 as i32]
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 2);

        cleanup(&state, event_id, &[sub1, sub2]).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn create_attendee_empty_subscribers() {
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
            .uri("/v1/kbr_event_attendees")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "kbr_event_id": 1,
                "mail_subscriber_ids": []
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn update_attendee_authenticated() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let event_id = seed_event(&state).await;
        let sub1 = seed_mail_subscriber(&state, &format!("UpdateSub1 {}", suffix())).await;
        seed_attendee(&state, event_id, sub1).await;

        let req = test::TestRequest::post()
            .uri("/v1/kbr_event_update_txt")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "kbr_event_id": event_id as i32,
                "text_copy": "Event updated!"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 1);

        cleanup(&state, event_id, &[sub1]).await;
    }
}
