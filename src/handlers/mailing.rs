use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_admin;
use crate::error::AppError;
use crate::models::mail_subscriber::{
    CreateMailSubscriberRequest, MailSubscriber, MailSubscriberResponse,
};

pub async fn index(user: CurrentUser) -> Result<HttpResponse, AppError> {
    if !is_admin(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let subscribers = mock_all_subscribers();
    let responses: Vec<MailSubscriberResponse> =
        subscribers.iter().map(|s| s.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn index_artist_subscribers(
    user: CurrentUser,
    query: web::Query<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    if !is_admin(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let artist_id = match query.0.get("artist_id") {
        Some(serde_json::Value::Number(n)) => n.as_i64().ok_or_else(|| {
            AppError::BadRequest("Invalid artist_id".to_string())
        })?,
        Some(serde_json::Value::String(s)) => s.parse::<i64>().map_err(|_| {
            AppError::BadRequest("Invalid artist_id".to_string())
        })?,
        _ => return Err(AppError::BadRequest("artist_id is required".to_string())),
    };
    let subscribers = mock_subscribers_for_artist(artist_id);
    let responses: Vec<MailSubscriberResponse> =
        subscribers.iter().map(|s| s.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn artist_mail_subscriber(
    user: CurrentUser,
    body: web::Json<CreateMailSubscriberRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_admin(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let artist_id = body.artist_id.ok_or_else(|| {
        AppError::BadRequest("artist_id is required".to_string())
    })?;

    if !MailSubscriber::validate_email(&body.email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    let existing = mock_find_by_email_and_artist(&body.email, artist_id);
    if existing.is_some() {
        return Err(AppError::UnprocessableEntity(
            "Email already subscribed to this artist".to_string(),
        ));
    }

    let subscriber = MailSubscriber {
        id: 999,
        full_name: body.full_name.clone(),
        email: body.email.clone(),
        active: Some(true),
        artist_id: Some(artist_id),
        unsubscribed_at: None,
        unsubscribe_token: Some(MailSubscriber::generate_unsubscribe_token()),
        user_id: Some(user.id),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(subscriber.to_response()))
}

pub async fn add_mail_subscriber_with_user(
    user: CurrentUser,
    body: web::Json<CreateMailSubscriberRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_admin(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let artist_id = body.artist_id.ok_or_else(|| {
        AppError::BadRequest("artist_id is required".to_string())
    })?;

    if !MailSubscriber::validate_email(&body.email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    let existing = mock_find_by_email_and_artist(&body.email, artist_id);
    if existing.is_some() {
        return Err(AppError::UnprocessableEntity(
            "Email already subscribed to this artist".to_string(),
        ));
    }

    let subscriber = MailSubscriber {
        id: 998,
        full_name: body.full_name.clone(),
        email: body.email.clone(),
        active: Some(true),
        artist_id: Some(artist_id),
        unsubscribed_at: None,
        unsubscribe_token: Some(MailSubscriber::generate_unsubscribe_token()),
        user_id: Some(user.id),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(subscriber.to_response()))
}

pub async fn add_mail_subscriber(
    body: web::Json<CreateMailSubscriberRequest>,
) -> Result<HttpResponse, AppError> {
    if !MailSubscriber::validate_email(&body.email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    let subscriber = MailSubscriber {
        id: 997,
        full_name: body.full_name.clone(),
        email: body.email.clone(),
        active: Some(true),
        artist_id: body.artist_id,
        unsubscribed_at: None,
        unsubscribe_token: Some(MailSubscriber::generate_unsubscribe_token()),
        user_id: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(subscriber.to_response()))
}

pub async fn unsubscribe(
    user: CurrentUser,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let artist_id = match body.get("artist_id") {
        Some(serde_json::Value::Number(n)) => n.as_i64().ok_or_else(|| {
            AppError::BadRequest("Invalid artist_id".to_string())
        })?,
        _ => return Err(AppError::BadRequest("artist_id is required".to_string())),
    };

    let found = mock_find_by_user_and_artist(user.id, artist_id);
    if found.is_none() {
        return Err(AppError::NotFound("Subscription not found".to_string()));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "success"
    })))
}

pub async fn request_unsubscribe(
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let email = match body.get("email") {
        Some(serde_json::Value::String(s)) => s.clone(),
        _ => return Err(AppError::UnprocessableEntity("Email is required".to_string())),
    };

    let found = mock_find_by_email(&email);
    if found.is_none() {
        return Err(AppError::NotFound("Email not found in our system".to_string()));
    }

    let token = MailSubscriber::generate_unsubscribe_token();
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Unsubscribe link generated",
        "token": token,
        "unsubscribe_url": format!("/v1/unsubscribe/{}", token)
    })))
}

pub async fn process_unsubscribe(path: web::Path<String>) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();
    if token.is_empty() {
        return Err(AppError::UnprocessableEntity("Token is required".to_string()));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Unsubscribed successfully",
        "status": "success"
    })))
}

fn mock_all_subscribers() -> Vec<MailSubscriber> {
    vec![MailSubscriber {
        id: 1,
        full_name: "John Doe".to_string(),
        email: "john@example.com".to_string(),
        active: Some(true),
        artist_id: Some(5),
        unsubscribed_at: None,
        unsubscribe_token: Some("token123".to_string()),
        user_id: Some(10),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_subscribers_for_artist(_artist_id: i64) -> Vec<MailSubscriber> {
    mock_all_subscribers()
}

fn mock_find_by_email_and_artist(
    _email: &str,
    _artist_id: i64,
) -> Option<MailSubscriber> {
    None
}

fn mock_find_by_user_and_artist(
    _user_id: i64,
    _artist_id: i64,
) -> Option<MailSubscriber> {
    Some(MailSubscriber {
        id: 1,
        full_name: "John Doe".to_string(),
        email: "john@example.com".to_string(),
        active: Some(true),
        artist_id: Some(5),
        unsubscribed_at: None,
        unsubscribe_token: Some("token123".to_string()),
        user_id: Some(10),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}

fn mock_find_by_email(_email: &str) -> Option<MailSubscriber> {
    Some(MailSubscriber {
        id: 1,
        full_name: "John Doe".to_string(),
        email: "john@example.com".to_string(),
        active: Some(true),
        artist_id: Some(5),
        unsubscribed_at: None,
        unsubscribe_token: Some("token123".to_string()),
        user_id: Some(10),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/mail_subscribers", web::get().to(index))
            .route("/artist_mailing_list", web::get().to(index_artist_subscribers))
            .route("/artistmailsubscriber", web::post().to(artist_mail_subscriber))
            .route("/addmailsubscriber", web::post().to(add_mail_subscriber))
            .route(
                "/addmailsubscriber_with_user",
                web::post().to(add_mail_subscriber_with_user),
            )
            .route("/mail_subscribers/unsubscribe", web::post().to(unsubscribe))
            .route("/unsubscribe", web::post().to(request_unsubscribe))
            .route("/unsubscribe/{token}", web::get().to(process_unsubscribe)),
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
    async fn mail_subscribers_index_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/mail_subscribers")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_mailing_list_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/artist_mailing_list?artist_id=5")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_mail_subscriber_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/addmailsubscriber")
            .set_json(serde_json::json!({
                "full_name": "Jane Doe",
                "email": "jane@example.com"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_mail_subscriber_invalid_email() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/addmailsubscriber")
            .set_json(serde_json::json!({
                "full_name": "Jane Doe",
                "email": "invalid"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_mail_subscriber_with_user_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/addmailsubscriber_with_user")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "full_name": "Jane Doe",
                "email": "jane@example.com",
                "artist_id": 5
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unsubscribe_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/mail_subscribers/unsubscribe")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "artist_id": 5
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_unsubscribe_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/unsubscribe")
            .set_json(serde_json::json!({
                "email": "john@example.com"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn process_unsubscribe_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/unsubscribe/sometoken")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }
}
