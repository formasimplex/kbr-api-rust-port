use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_artist_or_above;
use crate::error::AppError;
use crate::models::kbr_event_attendee::{
    CreateEventAttendeeRequest, KbrEventAttendee, KbrEventAttendeeResponse,
    UpdateEventAttendeeRequest,
};

pub async fn qr_scan(path: web::Path<i64>) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_attendee(id) {
        Some(mut attendee) => {
            let new_count = attendee.scan_count.unwrap_or(0) + 1;
            attendee.scan_count = Some(new_count);
            Ok(HttpResponse::Ok().json(attendee.to_response()))
        }
        None => Err(AppError::NotFound(format!("Attendee #{}", id))),
    }
}

pub async fn attendees_for_event(
    user: CurrentUser,
    query: web::Query<serde_json::Value>,
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
    let attendees = mock_attendees_for_event(event_id as i32);
    let responses: Vec<KbrEventAttendeeResponse> =
        attendees.iter().map(|a| a.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn create(
    user: CurrentUser,
    body: web::Json<CreateEventAttendeeRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    if body.mail_subscriber_ids.is_empty() {
        return Err(AppError::Validation(
            "mail_subscriber_ids is required".to_string(),
        ));
    }
    let attendees: Vec<KbrEventAttendee> = body
        .mail_subscriber_ids
        .iter()
        .enumerate()
        .map(|(i, subscriber_id)| KbrEventAttendee {
            id: 1000 + i as i64,
            kbr_event_id: Some(body.kbr_event_id),
            mail_subscriber_id: Some(*subscriber_id),
            scan_count: Some(0),
            headcount: body.headcount,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
        .collect();
    let responses: Vec<KbrEventAttendeeResponse> =
        attendees.iter().map(|a| a.to_response()).collect();
    Ok(HttpResponse::Created().json(responses))
}

pub async fn update(
    user: CurrentUser,
    body: web::Json<UpdateEventAttendeeRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let attendees = mock_attendees_for_event(body.kbr_event_id);
    let responses: Vec<KbrEventAttendeeResponse> =
        attendees.iter().map(|a| a.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

fn mock_find_attendee(id: i64) -> Option<KbrEventAttendee> {
    if id == 1 {
        Some(KbrEventAttendee {
            id: 1,
            kbr_event_id: Some(5),
            mail_subscriber_id: Some(10),
            scan_count: Some(2),
            headcount: Some(3),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    } else {
        None
    }
}

fn mock_attendees_for_event(_event_id: i32) -> Vec<KbrEventAttendee> {
    vec![
        KbrEventAttendee {
            id: 1,
            kbr_event_id: Some(5),
            mail_subscriber_id: Some(10),
            scan_count: Some(2),
            headcount: Some(3),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        KbrEventAttendee {
            id: 2,
            kbr_event_id: Some(5),
            mail_subscriber_id: Some(11),
            scan_count: Some(0),
            headcount: Some(1),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
    ]
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
    use crate::auth::jwt::encode_token;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";

    fn admin_token() -> String {
        crate::auth::jwt::encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn qr_scan_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/qr_scan/1").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn qr_scan_not_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/qr_scan/9999").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn attendees_for_event_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/kbr_event_attendees?kbr_event_id=5")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn attendees_for_event_forbidden() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/kbr_event_attendees?kbr_event_id=5")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert!(resp.status().is_client_error());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn create_attendee_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/kbr_event_attendees")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "kbr_event_id": 5,
                "mail_subscriber_ids": [10, 11]
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn create_attendee_empty_subscribers() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/kbr_event_attendees")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "kbr_event_id": 5,
                "mail_subscriber_ids": []
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn update_attendee_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/kbr_event_update_txt")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "kbr_event_id": 5,
                "text_copy": "Event updated!"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }
}
