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
use std::time::Duration;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_artist_or_above;
use crate::data::event_attendees as data;
use crate::error::AppError;
use crate::jobs::Job;
use crate::models::kbr_event_attendee::{
    CreateEventAttendeeRequest, KbrEventAttendeeResponse,
    UpdateEventAttendeeRequest,
};

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

    match data::qr_scan(&state.db, id).await? {
        Some(attendee) => Ok(HttpResponse::Ok().json(attendee.to_response())),
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

    let attendees = data::list_by_event(&state.db, event_id as i32).await?;
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
        let attendee = data::create_one(&mut tx, body.kbr_event_id, *subscriber_id, body.headcount, now).await?;
        attendees.push(attendee);
    }

    tx.commit().await?;

    // Enqueue QR code email for each attendee (batched to avoid channel saturation)
    const JOB_BATCH_SIZE: usize = 250;
    for chunk in attendees.chunks(JOB_BATCH_SIZE) {
        for attendee in chunk {
            let job_id = Uuid::new_v4();
            if let Err(e) = state
                .job_handle
                .send(Job::SendEventAttendeeEmail {
                    job_id,
                    attendee_id: attendee.id,
                    event_id: attendee.kbr_event_id.map(|id| id as i64).unwrap_or(0),
                })
                .await
            {
                tracing::warn!(
                    job_id = %job_id,
                    attendee_id = attendee.id,
                    error = %e,
                    "Failed to enqueue event attendee email job"
                );
            }
        }
        if chunk.len() == JOB_BATCH_SIZE {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

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

    let attendees = data::list_by_event(&state.db, body.kbr_event_id).await?;

    // Enqueue text update email for each attendee (batched to avoid channel saturation)
    const JOB_BATCH_SIZE: usize = 250;
    for chunk in attendees.chunks(JOB_BATCH_SIZE) {
        for attendee in chunk {
            let job_id = Uuid::new_v4();
            if let Err(e) = state
                .job_handle
                .send(Job::SendEventUpdateEmail {
                    job_id,
                    attendee_id: attendee.id,
                    text_copy: body.text_copy.clone(),
                })
                .await
            {
                tracing::warn!(
                    job_id = %job_id,
                    attendee_id = attendee.id,
                    error = %e,
                    "Failed to enqueue event update email job"
                );
            }
        }
        if chunk.len() == JOB_BATCH_SIZE {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    let responses: Vec<KbrEventAttendeeResponse> =
        attendees.iter().map(|a| a.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/qr_scan/{id}", web::get().to(qr_scan))
        .route("/kbr_event_attendees", web::get().to(attendees_for_event))
        .route("/kbr_event_attendees", web::post().to(create))
        .route("/kbr_event_update_txt", web::post().to(update));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{admin_token, seed_attendee, unique_suffix};
    use actix_web::test;

    async fn seed_event(state: &AppState) -> i64 {
        let name = format!("Test Event {}", unique_suffix());
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

    #[tokio::test(flavor = "current_thread")]
    async fn qr_scan_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let event_id = seed_event(&state).await;
        let sub_id = seed_mail_subscriber(&state, &format!("QRScanTest {}", unique_suffix())).await;
        let attendee_id = seed_attendee(&state.db, event_id, sub_id, 0).await;

        let req = test::TestRequest::get()
            .uri(&format!("/qr_scan/{}", attendee_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["scan_count"], 1);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn qr_scan_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/qr_scan/99999999")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn attendees_for_event_authenticated() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let event_id = seed_event(&state).await;
        let sub1 = seed_mail_subscriber(&state, &format!("EventSub1 {}", unique_suffix())).await;
        let sub2 = seed_mail_subscriber(&state, &format!("EventSub2 {}", unique_suffix())).await;
        seed_attendee(&state.db, event_id, sub1, 0).await;
        seed_attendee(&state.db, event_id, sub2, 0).await;

        let req = test::TestRequest::get()
            .uri(&format!("/kbr_event_attendees?kbr_event_id={}", event_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert_eq!(body.len(), 2);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn attendees_for_event_forbidden() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/kbr_event_attendees?kbr_event_id=1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_client_error());

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn create_attendee_authenticated() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let event_id = seed_event(&state).await;
        let sub1 = seed_mail_subscriber(&state, &format!("CreateSub1 {}", unique_suffix())).await;
        let sub2 = seed_mail_subscriber(&state, &format!("CreateSub2 {}", unique_suffix())).await;

        let req = test::TestRequest::post()
            .uri("/kbr_event_attendees")
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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn create_attendee_empty_subscribers() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/kbr_event_attendees")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "kbr_event_id": 1,
                "mail_subscriber_ids": []
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn update_attendee_authenticated() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let event_id = seed_event(&state).await;
        let sub1 = seed_mail_subscriber(&state, &format!("UpdateSub1 {}", unique_suffix())).await;
        seed_attendee(&state.db, event_id, sub1, 0).await;

        let req = test::TestRequest::post()
            .uri("/kbr_event_update_txt")
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

        _guard.cleanup().await;
    }
}
