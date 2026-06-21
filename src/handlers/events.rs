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

use actix_web::{HttpResponse, web};

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_artist_or_above;
use crate::data::events as data;
use crate::error::AppError;
use crate::models::comment::{CommentResponse, CommentUser};
use crate::models::kbr_event::{
    CreateKbrEventRequest, KbrEvent, KbrEventResponse, UpdateKbrEventRequest,
};
use crate::services::storage_service;

/// Fetch comments for a set of event IDs in a single query with user JOIN.
///
/// Returns a HashMap mapping event_id -> Vec<CommentResponse>.
async fn fetch_event_comments_batch(
    pool: &sqlx::PgPool,
    event_ids: &[i64],
) -> std::collections::HashMap<i64, Vec<CommentResponse>> {
    if event_ids.is_empty() {
        return std::collections::HashMap::new();
    }
    let rows = sqlx::query_as::<_, (i64, Option<String>, chrono::NaiveDateTime, Option<String>, i64)>(
        r"SELECT c.id, c.content, c.created_at AT TIME ZONE 'UTC' AS created_at,
                 u.username, c.commentable_id
           FROM comments c
           LEFT JOIN users u ON u.id = c.user_id
           WHERE c.commentable_type = 'KBREvent' AND c.commentable_id = ANY($1)
           ORDER BY c.commentable_id, c.created_at",
    )
    .bind(event_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut map = std::collections::HashMap::new();
    for (id, content, created_at, username, commentable_id) in rows {
        map.entry(commentable_id)
            .or_insert_with(Vec::new)
            .push(CommentResponse {
                id,
                content,
                created_at: created_at.and_utc(),
                user: username.map(|u| CommentUser { username: Some(u) }),
                replies: Vec::new(),
            });
    }
    map
}

/// List all events.
///
/// Returns all events ordered by ID. No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `KbrEventResponse`
pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let events = data::list(&state.db).await?;
    let event_ids: Vec<i64> = events.iter().map(|e| e.id).collect();
    let comments_map = fetch_event_comments_batch(&state.db, &event_ids).await;
    let mut responses: Vec<KbrEventResponse> = Vec::new();
    for event in &events {
        let (image_urls, thumbnail_urls) =
            storage_service::get_image_urls(&state.s3, &state.db, "KbrEvent", event.id)
                .await
                .unwrap_or_else(|_| (Vec::new(), Vec::new()));
        let comments = comments_map.get(&event.id).cloned().unwrap_or_default();
        responses.push(event.to_response(image_urls, thumbnail_urls, comments));
    }
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

    match data::by_id(&state.db, id).await? {
        Some(event) => {
            let (image_urls, thumbnail_urls) =
                storage_service::get_image_urls(&state.s3, &state.db, "KbrEvent", event.id)
                    .await
                    .unwrap_or_else(|_| (Vec::new(), Vec::new()));
            let comments_map = fetch_event_comments_batch(&state.db, &[event.id]).await;
            let comments = comments_map.get(&event.id).cloned().unwrap_or_default();
            Ok(HttpResponse::Ok().json(event.to_response(image_urls, thumbnail_urls, comments)))
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
    let events = if user.is_admin() {
        data::list(&state.db).await?
    } else if is_artist_or_above(&user.role) {
        data::list_by_user(&state.db, user.id).await?
    } else {
        return Err(AppError::Forbidden(
            "Hey you! Yes you! You Are Not Authorized".to_string(),
        ));
    };
    let event_ids: Vec<i64> = events.iter().map(|e| e.id).collect();
    let comments_map = fetch_event_comments_batch(&state.db, &event_ids).await;
    let mut responses: Vec<KbrEventResponse> = Vec::new();
    for event in &events {
        let (image_urls, thumbnail_urls) =
            storage_service::get_image_urls(&state.s3, &state.db, "KbrEvent", event.id)
                .await
                .unwrap_or_else(|_| (Vec::new(), Vec::new()));
        let comments = comments_map.get(&event.id).cloned().unwrap_or_default();
        responses.push(event.to_response(image_urls, thumbnail_urls, comments));
    }
    Ok(HttpResponse::Ok().json(responses))
}

/// Create a new event.
///
/// Requires artist+ role. Validates that name, description, and dates are
/// provided. Strips carriage returns and newlines from the description.
/// Validates and checks ticket_url and external_url against Safe Browsing.
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
    if !KbrEvent::validate_required(&body.name, &body.description) {
        return Err(AppError::Validation(
            "name, description, required".to_string(),
        ));
    }

    for url in body.ticket_url.iter().chain(body.external_url.iter()) {
        if !KbrEvent::validate_url(url) {
            return Err(AppError::Validation(format!(
                "Invalid URL format: {}. Must start with http:// or https://",
                url
            )));
        }
        if let Some(ref sb) = state.safe_browsing
            && sb.is_malicious(url).await
        {
            return Err(AppError::UnprocessableEntity(
                "The URL provided is flagged as potentially malicious".to_string(),
            ));
        }
    }

    let now = chrono::Utc::now().naive_utc();

    let event = data::create(&state.db, &body.name, Some(&body.description.replace(['\r', '\n'], "")), true, Some(body.event_start_date.naive_utc()), Some(body.event_end_date.naive_utc()), user.id as i32, body.event_url.as_deref(), body.qr_encode_string.as_deref(), body.ticket_url.as_deref(), body.external_url.as_deref(), now).await?;
    Ok(HttpResponse::Created().json(event.to_response(Vec::new(), Vec::new(), Vec::new())))
}

/// Update an existing event.
///
/// Requires artist+ role. Updates name, description, active status, ticket URL,
/// and external URL using COALESCE for partial updates. Strips carriage returns
/// and newlines from name and description. Validates and checks ticket_url and
/// external_url against Safe Browsing.
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

    for url in body.ticket_url.iter().chain(body.external_url.iter()) {
        if !KbrEvent::validate_url(url) {
            return Err(AppError::Validation(format!(
                "Invalid URL format: {}. Must start with http:// or https://",
                url
            )));
        }
        if let Some(ref sb) = state.safe_browsing
            && sb.is_malicious(url).await
        {
            return Err(AppError::UnprocessableEntity(
                "The URL provided is flagged as potentially malicious".to_string(),
            ));
        }
    }

    let now = chrono::Utc::now().naive_utc();

    let cleaned_name = body.name.as_deref().map(|s| s.replace(['\r', '\n'], ""));
    let cleaned_desc = body.description.as_deref().map(|s| s.replace(['\r', '\n'], ""));
    let event = data::update(&state.db, id, cleaned_name.as_deref(), cleaned_desc.as_deref(), body.active, None, None, None, None, body.ticket_url.as_deref(), body.external_url.as_deref(), now).await?
        .ok_or_else(|| AppError::NotFound(format!("Event #{}", id)))?;
    Ok(HttpResponse::Ok().json(event.to_response(Vec::new(), Vec::new(), Vec::new())))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/kbrevents", web::get().to(index))
        .route("/kbrevent/{id}", web::get().to(show))
        .route("/kbr_events_by_user", web::get().to(index_by_user))
        .route("/kbrevents", web::post().to(create))
        .route("/kbrevents/{id}", web::put().to(update));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token_with_role;
    use crate::test_utils::{admin_token, artist_token, not_found_id, seed_test_user, unique_suffix, TEST_SECRET};
    use actix_web::test;

    async fn seed_event(pool: &sqlx::PgPool, create_by_user_id: i32) -> i64 {
        let now = chrono::Utc::now().naive_utc();
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO kbr_events (name, description, active, event_start_date, event_end_date,
               create_by_user_id, ticket_url, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING id",
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
        .fetch_one(pool)
        .await
        .expect("Failed to seed event")
    }

    #[tokio::test(flavor = "current_thread")]
    async fn events_index_public() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get().uri("/kbrevents").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_array());
        if body.as_array().unwrap().len() > 0 {
            assert!(body[0]["comments"].is_array());
        }

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_show_found() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, user_email) = seed_test_user(&state.db, "event_test", "artist").await;
        let event_id = seed_event(&state.db, user_id as i32).await;

        let req = test::TestRequest::get()
            .uri(&format!("/kbrevent/{}", event_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], event_id);
        assert!(body["comments"].is_array());
        if body["comments"].as_array().unwrap().len() > 0 {
            assert!(body["comments"][0]["replies"].is_array());
        }

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_show_not_found() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "kbr_events").await;

        let req = test::TestRequest::get()
            .uri(&format!("/kbrevent/{}", not_found))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_create_authenticated() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let name = format!("New Event {}", unique_suffix());
        let now = chrono::Utc::now();

        let req = test::TestRequest::post()
            .uri("/kbrevents")
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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_create_forbidden_non_artist() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let token = encode_token_with_role(2, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let now = chrono::Utc::now();
        let req = test::TestRequest::post()
            .uri("/kbrevents")
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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn events_by_user_admin() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, user_email) = seed_test_user(&state.db, "event_test", "artist").await;
        let event_id = seed_event(&state.db, user_id as i32).await;

        let req = test::TestRequest::get()
            .uri("/kbr_events_by_user")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_array());
        if body.as_array().unwrap().len() > 0 {
            assert!(body[0]["comments"].is_array());
        }

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn events_by_user_artist() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, user_email) = seed_test_user(&state.db, "event_test", "artist").await;
        let event_id = seed_event(&state.db, user_id as i32).await;

        let req = test::TestRequest::get()
            .uri("/kbr_events_by_user")
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_array());
        if body.as_array().unwrap().len() > 0 {
            assert!(body[0]["comments"].is_array());
        }

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn events_by_user_forbidden() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let token = encode_token_with_role(2, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let req = test::TestRequest::get()
            .uri("/kbr_events_by_user")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_update_success() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, user_email) = seed_test_user(&state.db, "event_test", "artist").await;
        let event_id = seed_event(&state.db, user_id as i32).await;

        let req = test::TestRequest::put()
            .uri(&format!("/kbrevents/{}", event_id))
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_update_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "kbr_events").await;

        let req = test::TestRequest::put()
            .uri(&format!("/kbrevents/{}", not_found))
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .set_json(serde_json::json!({
                "name": "Updated"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }
}
