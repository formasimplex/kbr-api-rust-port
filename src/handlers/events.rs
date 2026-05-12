use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_artist_or_above;
use crate::error::AppError;
use crate::models::kbr_event::{CreateKbrEventRequest, KbrEvent, KbrEventResponse, UpdateKbrEventRequest};

pub async fn index() -> Result<HttpResponse, AppError> {
    let events = mock_all_events();
    let responses: Vec<KbrEventResponse> = events.iter().map(|e| e.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(path: web::Path<i64>) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_event(id) {
        Some(e) => Ok(HttpResponse::Ok().json(e.to_response())),
        None => Err(AppError::NotFound(format!("Event #{}", id))),
    }
}

pub async fn index_by_user(user: CurrentUser) -> Result<HttpResponse, AppError> {
    let events = if user.is_admin() {
        mock_all_events()
    } else if is_artist_or_above(&user.role) {
        mock_user_events(user.id)
    } else {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    };
    let responses: Vec<KbrEventResponse> = events.iter().map(|e| e.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn create(
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
    let event = KbrEvent {
        id: 999,
        name: Some(body.name.clone()),
        description: Some(body.description.replace('\r', "").replace('\n', "")),
        active: Some(true),
        event_start_date: Some(body.event_start_date),
        event_end_date: Some(body.event_end_date),
        create_by_user_id: Some(user.id as i32),
        event_url: body.event_url.clone(),
        qr_encode_string: body.qr_encode_string.clone(),
        ticket_url: body.ticket_url.clone(),
        external_url: body.external_url.clone(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(event.to_response()))
}

pub async fn update(
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateKbrEventRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();
    match mock_find_event(id) {
        Some(mut e) => {
            if let Some(ref name) = body.name {
                e.name = Some(name.clone());
            }
            if let Some(ref desc) = body.description {
                e.description = Some(desc.replace('\r', "").replace('\n', ""));
            }
            if let Some(active) = body.active {
                e.active = Some(active);
            }
            if let Some(ref url) = body.ticket_url {
                e.ticket_url = Some(url.clone());
            }
            if let Some(ref url) = body.external_url {
                e.external_url = Some(url.clone());
            }
            Ok(HttpResponse::Ok().json(e.to_response()))
        }
        None => Err(AppError::NotFound(format!("Event #{}", id))),
    }
}

fn mock_all_events() -> Vec<KbrEvent> {
    vec![KbrEvent {
        id: 1,
        name: Some("Summer Festival".to_string()),
        description: Some("A great festival".to_string()),
        active: Some(true),
        event_start_date: Some(chrono::Utc::now()),
        event_end_date: Some(chrono::Utc::now()),
        create_by_user_id: Some(1),
        event_url: None,
        qr_encode_string: None,
        ticket_url: Some("https://tickets.com/1".to_string()),
        external_url: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_user_events(_user_id: i64) -> Vec<KbrEvent> {
    mock_all_events()
}

fn mock_find_event(id: i64) -> Option<KbrEvent> {
    if id == 1 {
        Some(KbrEvent {
            id: 1,
            name: Some("Summer Festival".to_string()),
            description: Some("A great festival".to_string()),
            active: Some(true),
            event_start_date: Some(chrono::Utc::now()),
            event_end_date: Some(chrono::Utc::now()),
            create_by_user_id: Some(1),
            event_url: None,
            qr_encode_string: None,
            ticket_url: Some("https://tickets.com/1".to_string()),
            external_url: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    } else {
        None
    }
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
    use crate::auth::jwt::encode_token;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";

    fn admin_token() -> String {
        crate::auth::jwt::encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn events_index_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/kbrevents").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_show_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/kbrevent/1").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_show_not_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/kbrevent/9999").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn event_create_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let now = chrono::Utc::now();
        let req = test::TestRequest::post()
            .uri("/v1/kbrevents")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "name": "New Event",
                "description": "A new event",
                "event_start_date": now.to_rfc3339(),
                "event_end_date": now.to_rfc3339()
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn events_by_user_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/kbr_events_by_user")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }
}
