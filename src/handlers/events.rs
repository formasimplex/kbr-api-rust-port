//! Event handlers
//!
//! Provides endpoints for event management including creation, listing,
//! and updates. Read endpoints are public; write operations require artist+ role.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/kbrevents` | public | List all events |
//! | `show` | GET | `/v1/kbrevent/{id}` | public | Retrieve a single event by ID |
//! | `index_by_user` | GET | `/v1/kbr_events_by_user` | artist+ | List events for current user |
//! | `create` | POST | `/v1/kbrevents` | artist+ | Create a new event |
//! | `update` | PUT | `/v1/kbrevents/{id}` | artist+ | Update an existing event |

use actix_web::{web, HttpResponse};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_artist_or_above;
use crate::error::AppError;
use crate::models::kbr_event::{CreateKbrEventRequest, KbrEvent, KbrEventResponse, UpdateKbrEventRequest};

#[derive(Debug, FromRow)]
struct KbrEventRow {
    id: i64,
    name: Option<String>,
    description: Option<String>,
    active: Option<bool>,
    event_start_date: Option<chrono::NaiveDateTime>,
    event_end_date: Option<chrono::NaiveDateTime>,
    create_by_user_id: Option<i32>,
    event_url: Option<String>,
    qr_encode_string: Option<String>,
    ticket_url: Option<String>,
    external_url: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<KbrEventRow> for KbrEvent {
    fn from(row: KbrEventRow) -> Self {
        KbrEvent {
            id: row.id,
            name: row.name,
            description: row.description,
            active: row.active,
            event_start_date: row.event_start_date.map(|dt| dt.and_utc()),
            event_end_date: row.event_end_date.map(|dt| dt.and_utc()),
            create_by_user_id: row.create_by_user_id,
            event_url: row.event_url,
            qr_encode_string: row.qr_encode_string,
            ticket_url: row.ticket_url,
            external_url: row.external_url,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

/// List all events.
///
/// Returns all events ordered by ID. No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `KbrEventResponse`
pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let rows = sqlx::query_as::<_, KbrEventRow>(
        r#"SELECT id, name, description, active, event_start_date, event_end_date,
           create_by_user_id, event_url, qr_encode_string, ticket_url, external_url,
           created_at, updated_at FROM kbr_events ORDER BY id"#
    )
    .fetch_all(&state.db)
    .await?;

    let events: Vec<KbrEvent> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<KbrEventResponse> = events.iter().map(|e| e.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Retrieve a single event by ID.
///
/// No authentication required.
///
/// # Response
///
/// `200 OK` — `KbrEventResponse`
/// `404 Not Found` — event does not exist
pub async fn show(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    match sqlx::query_as::<_, KbrEventRow>(
        r#"SELECT id, name, description, active, event_start_date, event_end_date,
           create_by_user_id, event_url, qr_encode_string, ticket_url, external_url,
           created_at, updated_at FROM kbr_events WHERE id = $1"#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let event: KbrEvent = row.into();
            Ok(HttpResponse::Ok().json(event.to_response()))
        }
        None => Err(AppError::NotFound(format!("Event #{}", id))),
    }
}

/// List events for the current user.
///
/// Admins see all events. Artists see only events they created.
/// Requires artist+ role.
///
/// # Response
///
/// `200 OK` — JSON array of `KbrEventResponse`
/// `403 Forbidden` — user lacks required role
pub async fn index_by_user(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    let rows = if user.is_admin() {
        sqlx::query_as::<_, KbrEventRow>(
            r#"SELECT id, name, description, active, event_start_date, event_end_date,
               create_by_user_id, event_url, qr_encode_string, ticket_url, external_url,
               created_at, updated_at FROM kbr_events ORDER BY id"#
        )
        .fetch_all(&state.db)
        .await?
    } else if is_artist_or_above(&user.role) {
        sqlx::query_as::<_, KbrEventRow>(
            r#"SELECT id, name, description, active, event_start_date, event_end_date,
               create_by_user_id, event_url, qr_encode_string, ticket_url, external_url,
               created_at, updated_at FROM kbr_events WHERE create_by_user_id = $1 ORDER BY id"#
        )
        .bind(user.id as i32)
        .fetch_all(&state.db)
        .await?
    } else {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    };

    let events: Vec<KbrEvent> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<KbrEventResponse> = events.iter().map(|e| e.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Create a new event.
///
/// Requires artist+ role. Validates that name, description, and dates are
/// provided. Strips carriage returns and newlines from the description.
///
/// # Response
///
/// `201 Created` — `KbrEventResponse`
/// `403 Forbidden` — user lacks required role
/// `422 Unprocessable` — validation failure
pub async fn create(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<CreateKbrEventRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    if !KbrEvent::validate_required(&body.name, &body.description, true, true) {
        return Err(AppError::Validation(
            "name, description, dates, and user are required".to_string(),
        ));
    }

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, KbrEventRow>(
        r#"INSERT INTO kbr_events (name, description, active, event_start_date, event_end_date,
           create_by_user_id, event_url, qr_encode_string, ticket_url, external_url, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
           RETURNING id, name, description, active, event_start_date, event_end_date,
           create_by_user_id, event_url, qr_encode_string, ticket_url, external_url, created_at, updated_at"#
    )
    .bind(&body.name)
    .bind(body.description.replace(['\r', '\n'], ""))
    .bind(true)
    .bind(body.event_start_date.naive_utc())
    .bind(body.event_end_date.naive_utc())
    .bind(Some(user.id as i32))
    .bind(body.event_url.as_deref())
    .bind(body.qr_encode_string.as_deref())
    .bind(body.ticket_url.as_deref())
    .bind(body.external_url.as_deref())
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let event: KbrEvent = row.into();
    Ok(HttpResponse::Created().json(event.to_response()))
}

/// Update an existing event.
///
/// Requires artist+ role. Updates name, description, active status, ticket URL,
/// and external URL using COALESCE for partial updates. Strips carriage returns
/// and newlines from name and description.
///
/// # Response
///
/// `200 OK` — `KbrEventResponse`
/// `403 Forbidden` — user lacks required role
/// `404 Not Found` — event does not exist
pub async fn update(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateKbrEventRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, KbrEventRow>(
        r#"UPDATE kbr_events SET
           name = COALESCE($1, name),
           description = COALESCE($2, description),
           active = COALESCE($3, active),
           ticket_url = COALESCE($4, ticket_url),
           external_url = COALESCE($5, external_url),
           updated_at = $6
           WHERE id = $7
           RETURNING id, name, description, active, event_start_date, event_end_date,
           create_by_user_id, event_url, qr_encode_string, ticket_url, external_url, created_at, updated_at"#
    )
    .bind(body.name.as_deref().map(|s| s.replace(['\r', '\n'], "")))
    .bind(body.description.as_deref().map(|s| s.replace(['\r', '\n'], "")))
    .bind(body.active)
    .bind(body.ticket_url.as_deref())
    .bind(body.external_url.as_deref())
    .bind(now)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Event #{}", id)))?;

    let event: KbrEvent = row.into();
    Ok(HttpResponse::Ok().json(event.to_response()))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/kbrevents", web::get().to(index))
            .route("/kbrevent/{id}", web::get().to(show))
            .route("/kbr_events_by_user", web::get().to(index_by_user))
            .route("/kbrevents", web::post().to(create))
            .route("/kbrevents/{id}", web::put().to(update)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token_with_role;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";
    const TEST_DB_URL: &str = "postgresql://ws@localhost:5432/kbr_test";

    use std::sync::atomic::{AtomicI64, Ordering};
    static TEST_COUNTER: AtomicI64 = AtomicI64::new(0);

    fn unique_suffix() -> String {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{}_{}", ts, count)
    }

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

        AppState { db: pool, s3, shopify: None }
    }

    fn admin_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    fn artist_token() -> String {
        encode_token_with_role(2, TEST_SECRET, 2, Some("artist".to_string())).unwrap()
    }

    async fn seed_user() -> (i64, String) {
        let email = format!("event_test_{}@test.com", unique_suffix());
        let id: i64 = sqlx::query_scalar(
            r"INSERT INTO users (email, password_digest, role, created_at, updated_at)
               VALUES ($1, $2, $3, NOW(), NOW())
               RETURNING id"
        )
        .bind(&email)
        .bind("hashed_password_test".to_string())
        .bind(Some("artist".to_string()))
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed user");
        (id, email)
    }

    async fn seed_event(create_by_user_id: i32) -> i64 {
        let now = chrono::Utc::now().naive_utc();
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO kbr_events (name, description, active, event_start_date, event_end_date,
               create_by_user_id, ticket_url, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING id"
        )
        .bind(format!("Test Event {}", unique_suffix()))
        .bind("A test event description")
        .bind(true)
        .bind(&now)
        .bind(&now)
        .bind(Some(create_by_user_id))
        .bind(Some("https://tickets.com/test".to_string()))
        .bind(&now)
        .bind(&now)
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed event")
    }

    async fn cleanup_event(event_id: i64) {
        let _ = sqlx::query(r"DELETE FROM kbr_events WHERE id = $1")
            .bind(event_id)
            .execute(&get_state().await.db)
            .await;
    }

    async fn cleanup_user(user_id: i64, email: &str) {
        let _ = sqlx::query(r"DELETE FROM kbr_events WHERE create_by_user_id = $1")
            .bind(user_id as i32)
            .execute(&get_state().await.db)
            .await;
        let _ = sqlx::query(r"DELETE FROM users WHERE email = $1")
            .bind(email)
            .execute(&get_state().await.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn events_index_public() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get().uri("/v1/kbrevents").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_show_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let (user_id, user_email) = seed_user().await;
        let event_id = seed_event(user_id as i32).await;

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/kbrevent/{}", event_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], event_id);

        cleanup_event(event_id).await;
        cleanup_user(user_id, &user_email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_show_not_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM kbr_events"
        )
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
            .uri(&format!("/v1/kbrevent/{}", max_id + 9999))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_create_authenticated() {
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

        let name = format!("New Event {}", unique_suffix());
        let now = chrono::Utc::now();

        let req = test::TestRequest::post()
            .uri("/v1/kbrevents")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "name": name,
                "description": "A new event",
                "event_start_date": now.to_rfc3339(),
                "event_end_date": now.to_rfc3339()
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["name"], name);

        let _ = sqlx::query(r"DELETE FROM kbr_events WHERE name = $1")
            .bind(&name)
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_create_forbidden_non_artist() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let token = encode_token_with_role(2, TEST_SECRET, 3, Some("user".to_string())).unwrap();
        let state = web::Data::new(get_state().await);

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let now = chrono::Utc::now();
        let req = test::TestRequest::post()
            .uri("/v1/kbrevents")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "name": "Should Fail",
                "description": "A new event",
                "event_start_date": now.to_rfc3339(),
                "event_end_date": now.to_rfc3339()
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn events_by_user_admin() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let (user_id, user_email) = seed_user().await;
        let event_id = seed_event(user_id as i32).await;

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/kbr_events_by_user")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        cleanup_event(event_id).await;
        cleanup_user(user_id, &user_email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn events_by_user_artist() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let (user_id, user_email) = seed_user().await;
        let event_id = seed_event(user_id as i32).await;

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/kbr_events_by_user")
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        cleanup_event(event_id).await;
        cleanup_user(user_id, &user_email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn events_by_user_forbidden() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let token = encode_token_with_role(2, TEST_SECRET, 3, Some("user".to_string())).unwrap();
        let state = web::Data::new(get_state().await);

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/kbr_events_by_user")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_update_success() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let (user_id, user_email) = seed_user().await;
        let event_id = seed_event(user_id as i32).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/v1/kbrevents/{}", event_id))
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .set_json(serde_json::json!({
                "name": "Updated Event Name",
                "active": false
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["name"], "Updated Event Name");
        assert_eq!(body["active"], false);

        cleanup_event(event_id).await;
        cleanup_user(user_id, &user_email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_update_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM kbr_events"
        )
        .fetch_one(&state.db)
        .await
        .expect("Failed to get max id");

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/v1/kbrevents/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .set_json(serde_json::json!({
                "name": "Updated"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }
}
