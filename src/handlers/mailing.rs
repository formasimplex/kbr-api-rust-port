//! Mailing list handlers
//!
//! Provides endpoints for managing mail subscribers, including listing,
//! subscribing, and unsubscribing. Most endpoints require authentication,
//! but some are public for unsubscribe flows.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/mail_subscribers` | admin | List all mail subscribers |
//! | `index_artist_subscribers` | GET | `/v1/artist_mailing_list` | admin | List subscribers for a specific artist by artist_id query param |
//! | `artist_mail_subscriber` | POST | `/v1/artistmailsubscriber` | admin | Create a subscriber for an artist (with user_id from auth) |
//! | `add_mail_subscriber_with_user` | POST | `/v1/addmailsubscriber_with_user` | admin | Subscribe an email to an artist's mailing list (with user_id from auth) |
//! | `add_mail_subscriber` | POST | `/v1/addmailsubscriber` | public | Subscribe an email to a mailing list |
//! | `unsubscribe` | POST | `/v1/mail_subscribers/unsubscribe` | auth | Unsubscribe the authenticated user from an artist's mailing list |
//! | `request_unsubscribe` | POST | `/v1/unsubscribe` | public | Generate an unsubscribe token for an email address |
//! | `process_unsubscribe` | GET | `/v1/unsubscribe/{token}` | public | Process an unsubscribe request with verification token |

use actix_web::{web, HttpResponse};
use sqlx::FromRow;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_admin;
use crate::data::mailing as data;
use crate::error::AppError;
use crate::jobs::Job;
use crate::models::mail_subscriber::{
    CreateMailSubscriberRequest, MailSubscriber, MailSubscriberResponse,
};

/// List all mail subscribers.
///
/// Requires admin role.
///
/// # Response
///
/// `200 OK` — JSON array of `MailSubscriberResponse`
/// `403 Forbidden` — insufficient role
pub async fn index(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    if !is_admin(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let subscribers = data::list(&state.db).await?;
    let responses: Vec<MailSubscriberResponse> =
        subscribers.iter().map(|s| s.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// List subscribers for a specific artist.
///
/// Requires admin role. The artist_id is passed as a query parameter.
///
/// # Response
///
/// `200 OK` — JSON array of `MailSubscriberResponse`
/// `403 Forbidden` — insufficient role
/// `400 Bad Request` — missing or invalid artist_id
pub async fn index_artist_subscribers(
    state: web::Data<AppState>,
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

    let subscribers = data::list_by_artist(&state.db, artist_id).await?;
    let responses: Vec<MailSubscriberResponse> =
        subscribers.iter().map(|s| s.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Create a subscriber for an artist.
///
/// Requires admin role. Associates the subscriber with the authenticated
/// user's ID. Validates email format and checks for duplicates.
///
/// # Response
///
/// `201 Created` — `MailSubscriberResponse` for the new subscriber
/// `403 Forbidden` — insufficient role
/// `400 Bad Request` — missing artist_id
/// `422 Unprocessable` — invalid email or duplicate subscription
pub async fn artist_mail_subscriber(
    state: web::Data<AppState>,
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

    if data::find_by_email_and_artist(&state.db, &body.email, artist_id).await?.is_some() {
        return Err(AppError::UnprocessableEntity(
            "Email already subscribed to this artist".to_string(),
        ));
    }

    let now = chrono::Utc::now().naive_utc();

    let subscriber = data::create(&state.db, &body.full_name, &body.email, Some(artist_id), Some(user.id), now).await?;
    Ok(HttpResponse::Created().json(subscriber.to_response()))
}

/// Subscribe an email to an artist's mailing list.
///
/// Requires admin role. Associates the subscriber with the authenticated
/// user's ID. Validates email format and checks for duplicates. Syncs with
/// Mailchimp if configured (non-fatal if Mailchimp fails).
///
/// # Response
///
/// `201 Created` — `MailSubscriberResponse` for the new subscriber
/// `200 OK` — subscriber created but Mailchimp sync failed (includes `mailchimp_error: true`)
/// `403 Forbidden` — insufficient role
/// `400 Bad Request` — missing artist_id
/// `422 Unprocessable` — invalid email or duplicate subscription
pub async fn add_mail_subscriber_with_user(
    state: web::Data<AppState>,
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

    if data::find_by_email_and_artist(&state.db, &body.email, artist_id).await?.is_some() {
        return Err(AppError::UnprocessableEntity(
            "Email already subscribed to this artist".to_string(),
        ));
    }

    let now = chrono::Utc::now().naive_utc();

    let subscriber = data::create(&state.db, &body.full_name, &body.email, Some(artist_id), Some(user.id), now).await?;

    if let Some(ref mc) = state.mailchimp
        && let Err(e) = mc.subscribe(&body.email, &body.full_name).await
    {
        tracing::warn!(error = %e, email = %body.email, "Mailchimp subscribe failed");
        return Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": 500,
            "message": e.to_string(),
            "subscriber": subscriber.to_response(),
            "mailchimp_error": true
        })));
    }

    Ok(HttpResponse::Created().json(subscriber.to_response()))
}

/// Subscribe an email to a mailing list.
///
/// Public endpoint — no authentication required. Validates email format.
/// Syncs with Mailchimp if configured (non-fatal if Mailchimp fails).
///
/// # Response
///
/// `201 Created` — `MailSubscriberResponse` for the new subscriber
/// `200 OK` — subscriber created but Mailchimp sync failed (includes `mailchimp_error: true`)
/// `422 Unprocessable` — invalid email
pub async fn add_mail_subscriber(
    state: web::Data<AppState>,
    body: web::Json<CreateMailSubscriberRequest>,
) -> Result<HttpResponse, AppError> {
    if !MailSubscriber::validate_email(&body.email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    let now = chrono::Utc::now().naive_utc();

    let subscriber = data::create(&state.db, &body.full_name, &body.email, body.artist_id, None, now).await?;

    if let Some(ref mc) = state.mailchimp
        && let Err(e) = mc.subscribe(&body.email, &body.full_name).await
    {
        tracing::warn!(error = %e, email = %body.email, "Mailchimp subscribe failed");
        return Ok(HttpResponse::Ok().json(serde_json::json!({
            "status": 500,
            "message": e.to_string(),
            "subscriber": subscriber.to_response(),
            "mailchimp_error": true
        })));
    }

    Ok(HttpResponse::Created().json(subscriber.to_response()))
}

/// Unsubscribe the authenticated user from an artist's mailing list.
///
/// Sets unsubscribed_at for the matching user_id/artist_id pair.
/// Requires authentication.
///
/// # Response
///
/// `200 OK` — success status
/// `400 Bad Request` — missing or invalid artist_id
/// `404 Not Found` — subscription not found
pub async fn unsubscribe(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let artist_id = match body.get("artist_id") {
        Some(serde_json::Value::Number(n)) => n.as_i64().ok_or_else(|| {
            AppError::BadRequest("Invalid artist_id".to_string())
        })?,
        _ => return Err(AppError::BadRequest("artist_id is required".to_string())),
    };

    let now = chrono::Utc::now().naive_utc();

    if !data::unsubscribe(&state.db, user.id, artist_id, now).await? {
        return Err(AppError::NotFound("Subscription not found".to_string()));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "success"
    })))
}

/// Generate an unsubscribe token for an email address.
///
/// Public endpoint — no authentication required. Validates that the
/// email exists in the system. Returns a token and unsubscribe URL.
///
/// # Response
///
/// `200 OK` — unsubscribe token and URL
/// `404 Not Found` — email not found
/// `422 Unprocessable` — email is required
pub async fn request_unsubscribe(
    state: web::Data<AppState>,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let email = match body.get("email") {
        Some(serde_json::Value::String(s)) => s.clone(),
        _ => return Err(AppError::UnprocessableEntity("Email is required".to_string())),
    };

    if !MailSubscriber::validate_email(&email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    if data::find_by_email(&state.db, &email).await?.is_some() {
        let job_id = Uuid::new_v4();
        let email_clone = email.clone();
        if let Err(e) = state.job_handle.send(Job::SendUnsubscribeEmail { job_id, email }).await {
            tracing::warn!(job_id = %job_id, email = %email_clone, error = %e, "Failed to enqueue unsubscribe email job");
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "If your email is registered, an unsubscribe link has been sent",
        "status": "success"
    })))
}

/// Process an unsubscribe request with verification token.
///
/// Public endpoint — no authentication required. Sets unsubscribed_at
/// for the subscriber matching the given token. Syncs with Mailchimp
/// if configured (non-fatal if Mailchimp fails).
///
/// # Response
///
/// `200 OK` — success confirmation
/// `404 Not Found` — invalid or expired token
/// `422 Unprocessable` — token is required
pub async fn process_unsubscribe(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();
    if token.is_empty() {
        return Err(AppError::UnprocessableEntity("Token is required".to_string()));
    }

    let now = chrono::Utc::now().naive_utc();

    match data::process_unsubscribe(&state.db, &token, now).await? {
        Some(email) => {
            if let Some(ref mc) = state.mailchimp
                && let Err(e) = mc.unsubscribe(&email).await {
                    tracing::warn!(error = %e, email = %email, "Mailchimp unsubscribe failed");
                }
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "message": "Unsubscribed successfully",
                "status": "success"
            })))
        }
        None => Err(AppError::NotFound("Invalid or expired unsubscribe token".to_string())),
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/mail_subscribers", web::get().to(index))
        .route("/artist_mailing_list", web::get().to(index_artist_subscribers))
        .route("/artistmailsubscriber", web::post().to(artist_mail_subscriber))
        .route("/addmailsubscriber", web::post().to(add_mail_subscriber))
        .route("/addmailsubscriber_with_user", web::post().to(add_mail_subscriber_with_user))
        .route("/mail_subscribers/unsubscribe", web::post().to(unsubscribe))
        .route("/unsubscribe", web::post().to(request_unsubscribe))
        .route("/unsubscribe/{token}", web::get().to(process_unsubscribe));
}

#[cfg(test)]
mod tests {
    use super::*;
 use crate::auth::jwt::encode_token_with_role;
     use crate::test_utils::{admin_token, seed_artist, seed_test_user, unique_suffix, TEST_SECRET};
    use actix_web::test;

    async fn seed_subscriber(pool: &sqlx::PgPool, artist_id: i64, user_id: Option<i64>) -> (i64, String) {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        let email = format!("mailing_sub_{}_{}@test.com", pid, ts);
        let token = MailSubscriber::generate_unsubscribe_token();
        let subscriber_id: i64 = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO mail_subscribers (full_name, email, active, artist_id, unsubscribe_token, user_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
               RETURNING id",
        )
        .bind(format!("Mailing Subscriber {}_{}", pid, ts))
        .bind(&email)
        .bind(true)
        .bind(artist_id)
        .bind(&token)
        .bind(user_id)
        .fetch_one(pool)
        .await
        .expect("Failed to seed subscriber");
        (subscriber_id, email)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mail_subscribers_index_authenticated() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/mail_subscribers")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mail_subscribers_index_forbidden() {
        crate::test_utils::set_test_env_jwt();
        let user_token = encode_token_with_role(99, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/mail_subscribers")
            .insert_header(("Authorization", format!("Bearer {}", user_token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_mailing_list_authenticated() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let (user_id, _) = seed_test_user(&state.db, "mailing_test", "admin").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;
        let (sub_id, _) = seed_subscriber(&state.db, artist_id, Some(user_id)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/artist_mailing_list?artist_id={}", artist_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        let arr = body.as_array().unwrap();
        let ids: Vec<i64> = arr.iter().map(|v| v["id"].as_i64().unwrap()).collect();
        assert!(ids.contains(&sub_id));

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_mail_subscriber_public() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        let email = format!("addmail_{}_{}@test.com", pid, ts);

        let req = test::TestRequest::post()
            .uri("/addmailsubscriber")
            .set_json(serde_json::json!({
                "full_name": "Jane Doe",
                "email": email.clone()
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["email"], email);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_mail_subscriber_invalid_email() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/addmailsubscriber")
            .set_json(serde_json::json!({
                "full_name": "Jane Doe",
                "email": "invalid"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_mail_subscriber_with_user_authenticated() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let (user_id, _) = seed_test_user(&state.db, "mailing_test", "admin").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let email = format!("addmailuser_{}@test.com", unique_suffix());

        let req = test::TestRequest::post()
            .uri("/addmailsubscriber_with_user")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "full_name": "Jane Doe",
                "email": email.clone(),
                "artist_id": artist_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["email"], email);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_mail_subscriber_duplicate() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let (user_id, _) = seed_test_user(&state.db, "mailing_test", "admin").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let email = format!("dupmail_{}@test.com", unique_suffix());

        sqlx::query(
            r"INSERT INTO mail_subscribers (full_name, email, active, artist_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, NOW(), NOW())",
        )
        .bind("Dup Test".to_string())
        .bind(&email)
        .bind(true)
        .bind(artist_id)
        .execute(&state.db)
        .await
        .expect("Failed to seed duplicate subscriber");

        let req = test::TestRequest::post()
            .uri("/artistmailsubscriber")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "full_name": "Jane Doe",
                "email": email.clone(),
                "artist_id": artist_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unsubscribe_authenticated() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let (user_id, _) = seed_test_user(&state.db, "mailing_test", "admin").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;
        let (sub_id, _) = seed_subscriber(&state.db, artist_id, Some(user_id)).await;
        let user_token = encode_token_with_role(user_id, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap();

        let req = test::TestRequest::post()
            .uri("/mail_subscribers/unsubscribe")
            .insert_header(("Authorization", format!("Bearer {}", user_token)))
            .set_json(serde_json::json!({
                "artist_id": artist_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let unsubscribed: Option<i64> = sqlx::query_scalar(
            r"SELECT id FROM mail_subscribers WHERE id = $1 AND unsubscribed_at IS NOT NULL",
        )
        .bind(sub_id)
        .fetch_optional(&state.db)
        .await
        .expect("Failed to check unsubscribe");
        assert_eq!(unsubscribed, Some(sub_id));

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unsubscribe_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/mail_subscribers/unsubscribe")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "artist_id": 99999
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_unsubscribe_public() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        let email = format!("requnsub_{}_{}@test.com", pid, ts);

        sqlx::query(
            r"INSERT INTO mail_subscribers (full_name, email, active, created_at, updated_at)
               VALUES ($1, $2, $3, NOW(), NOW())",
        )
        .bind("Req Unsub Test".to_string())
        .bind(&email)
        .bind(true)
        .execute(&state.db)
        .await
        .expect("Failed to seed subscriber");

        let req = test::TestRequest::post()
            .uri("/unsubscribe")
            .set_json(serde_json::json!({
                "email": email.clone()
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "success");
        assert!(body["message"].is_string());

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_unsubscribe_not_found_returns_same_response() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/unsubscribe")
            .set_json(serde_json::json!({
                "email": "nonexistent@test.com"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "success");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn process_unsubscribe_public() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        let email = format!("procunsub_{}_{}@test.com", pid, ts);
        let token = MailSubscriber::generate_unsubscribe_token();

        let sub_id: i64 = sqlx::query_scalar(
            r"INSERT INTO mail_subscribers (full_name, email, active, unsubscribe_token, created_at, updated_at)
               VALUES ($1, $2, $3, $4, NOW(), NOW())
               RETURNING id",
        )
        .bind("Proc Unsub Test".to_string())
        .bind(&email)
        .bind(true)
        .bind(&token)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed subscriber");

        let req = test::TestRequest::get()
            .uri(&format!("/unsubscribe/{}", token))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let unsubscribed: Option<i64> = sqlx::query_scalar(
            r"SELECT id FROM mail_subscribers WHERE id = $1 AND unsubscribed_at IS NOT NULL",
        )
        .bind(sub_id)
        .fetch_optional(&state.db)
        .await
        .expect("Failed to check unsubscribe");
        assert_eq!(unsubscribed, Some(sub_id));

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn process_unsubscribe_invalid_token() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/unsubscribe/invalidtoken123")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }
}
