use actix_web::{web, HttpResponse};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_admin;
use crate::error::AppError;
use crate::models::mail_subscriber::{
    CreateMailSubscriberRequest, MailSubscriber, MailSubscriberResponse,
};

#[derive(Debug, FromRow)]
struct MailSubscriberRow {
    id: i64,
    full_name: String,
    email: String,
    active: Option<bool>,
    artist_id: Option<i64>,
    unsubscribed_at: Option<chrono::NaiveDateTime>,
    unsubscribe_token: Option<String>,
    user_id: Option<i64>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<MailSubscriberRow> for MailSubscriber {
    fn from(row: MailSubscriberRow) -> Self {
        MailSubscriber {
            id: row.id,
            full_name: row.full_name,
            email: row.email,
            active: row.active,
            artist_id: row.artist_id,
            unsubscribed_at: row.unsubscribed_at.map(|dt| dt.and_utc()),
            unsubscribe_token: row.unsubscribe_token,
            user_id: row.user_id,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const SUBSCRIBER_SELECT: &str =
    r#"SELECT id, full_name, email, active, artist_id, unsubscribed_at,
       unsubscribe_token, user_id, created_at, updated_at FROM mail_subscribers"#;

pub async fn index(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    if !is_admin(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let rows = sqlx::query_as::<_, MailSubscriberRow>(
        &format!("{} ORDER BY id", SUBSCRIBER_SELECT),
    )
    .fetch_all(&state.db)
    .await?;

    let subscribers: Vec<MailSubscriber> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<MailSubscriberResponse> =
        subscribers.iter().map(|s| s.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

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

    let rows = sqlx::query_as::<_, MailSubscriberRow>(
        &format!("{} WHERE artist_id = $1 ORDER BY id", SUBSCRIBER_SELECT),
    )
    .bind(artist_id)
    .fetch_all(&state.db)
    .await?;

    let subscribers: Vec<MailSubscriber> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<MailSubscriberResponse> =
        subscribers.iter().map(|s| s.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

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

    let existing = sqlx::query_as::<_, MailSubscriberRow>(
        &format!("{} WHERE email = $1 AND artist_id = $2", SUBSCRIBER_SELECT),
    )
    .bind(&body.email)
    .bind(artist_id)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_some() {
        return Err(AppError::UnprocessableEntity(
            "Email already subscribed to this artist".to_string(),
        ));
    }

    let token = MailSubscriber::generate_unsubscribe_token();
    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, MailSubscriberRow>(
        r#"INSERT INTO mail_subscribers (full_name, email, active, artist_id, unsubscribe_token, user_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING id, full_name, email, active, artist_id, unsubscribed_at,
               unsubscribe_token, user_id, created_at, updated_at"#,
    )
    .bind(&body.full_name)
    .bind(&body.email)
    .bind(true)
    .bind(artist_id)
    .bind(&token)
    .bind(user.id)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let subscriber: MailSubscriber = row.into();
    Ok(HttpResponse::Created().json(subscriber.to_response()))
}

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

    let existing = sqlx::query_as::<_, MailSubscriberRow>(
        &format!("{} WHERE email = $1 AND artist_id = $2", SUBSCRIBER_SELECT),
    )
    .bind(&body.email)
    .bind(artist_id)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_some() {
        return Err(AppError::UnprocessableEntity(
            "Email already subscribed to this artist".to_string(),
        ));
    }

    let token = MailSubscriber::generate_unsubscribe_token();
    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, MailSubscriberRow>(
        r#"INSERT INTO mail_subscribers (full_name, email, active, artist_id, unsubscribe_token, user_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING id, full_name, email, active, artist_id, unsubscribed_at,
               unsubscribe_token, user_id, created_at, updated_at"#,
    )
    .bind(&body.full_name)
    .bind(&body.email)
    .bind(true)
    .bind(artist_id)
    .bind(&token)
    .bind(user.id)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let subscriber: MailSubscriber = row.into();
    Ok(HttpResponse::Created().json(subscriber.to_response()))
}

pub async fn add_mail_subscriber(
    state: web::Data<AppState>,
    body: web::Json<CreateMailSubscriberRequest>,
) -> Result<HttpResponse, AppError> {
    if !MailSubscriber::validate_email(&body.email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    let token = MailSubscriber::generate_unsubscribe_token();
    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, MailSubscriberRow>(
        r#"INSERT INTO mail_subscribers (full_name, email, active, artist_id, unsubscribe_token, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id, full_name, email, active, artist_id, unsubscribed_at,
               unsubscribe_token, user_id, created_at, updated_at"#,
    )
    .bind(&body.full_name)
    .bind(&body.email)
    .bind(true)
    .bind(body.artist_id)
    .bind(&token)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let subscriber: MailSubscriber = row.into();
    Ok(HttpResponse::Created().json(subscriber.to_response()))
}

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

    let result = sqlx::query(
        r#"UPDATE mail_subscribers SET unsubscribed_at = $1, updated_at = $2
           WHERE user_id = $3 AND artist_id = $4 AND unsubscribed_at IS NULL"#,
    )
    .bind(now)
    .bind(now)
    .bind(user.id)
    .bind(artist_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Subscription not found".to_string()));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "status": "success"
    })))
}

pub async fn request_unsubscribe(
    state: web::Data<AppState>,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let email = match body.get("email") {
        Some(serde_json::Value::String(s)) => s.clone(),
        _ => return Err(AppError::UnprocessableEntity("Email is required".to_string())),
    };

    let existing = sqlx::query_as::<_, MailSubscriberRow>(
        &format!("{} WHERE email = $1", SUBSCRIBER_SELECT),
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_none() {
        return Err(AppError::NotFound("Email not found in our system".to_string()));
    }

    let token = MailSubscriber::generate_unsubscribe_token();
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Unsubscribe link generated",
        "token": token,
        "unsubscribe_url": format!("/v1/unsubscribe/{}", token)
    })))
}

pub async fn process_unsubscribe(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();
    if token.is_empty() {
        return Err(AppError::UnprocessableEntity("Token is required".to_string()));
    }

    let now = chrono::Utc::now().naive_utc();

    let result = sqlx::query(
        r#"UPDATE mail_subscribers SET unsubscribed_at = $1, updated_at = $2
           WHERE unsubscribe_token = $3 AND unsubscribed_at IS NULL"#,
    )
    .bind(now)
    .bind(now)
    .bind(&token)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Invalid or expired unsubscribe token".to_string()));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Unsubscribed successfully",
        "status": "success"
    })))
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

        AppState { db: pool, s3 }
    }

    fn admin_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    async fn seed_user() -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO users (email, password_digest, role, created_at, updated_at)
               VALUES ($1, $2, $3, NOW(), NOW())
               RETURNING id",
        )
        .bind(format!("mailing_test_user_{}_{}@test.com", pid, ts))
        .bind("hashed_password_test".to_string())
        .bind(Some("admin".to_string()))
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed user")
    }

    async fn seed_artist(user_id: i64) -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO artists (name, genre, bio, user_id, prospect, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               RETURNING id",
        )
        .bind(format!("Mailing Test Artist {}_{}", pid, ts))
        .bind(Some("Electronic".to_string()))
        .bind(Some("A test artist for mailing".to_string()))
        .bind(Some(user_id))
        .bind(Some(false))
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed artist")
    }

    async fn seed_subscriber(artist_id: i64, user_id: Option<i64>) -> (i64, String) {
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
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed subscriber");
        (subscriber_id, email)
    }

    async fn cleanup_subscriber(subscriber_id: i64) {
        let _ = sqlx::query(r"DELETE FROM mail_subscribers WHERE id = $1")
            .bind(subscriber_id)
            .execute(&get_state().await.db)
            .await;
    }

    async fn cleanup_artist(artist_id: i64) {
        let subscribers: Vec<i64> = sqlx::query_scalar(
            r"SELECT id FROM mail_subscribers WHERE artist_id = $1",
        )
        .bind(artist_id)
        .fetch_all(&get_state().await.db)
        .await
        .unwrap_or_default();
        for sid in subscribers {
            cleanup_subscriber(sid).await;
        }
        let _ = sqlx::query(r"DELETE FROM artists WHERE id = $1")
            .bind(artist_id)
            .execute(&get_state().await.db)
            .await;
    }

    async fn cleanup_user(user_id: i64) {
        let artists: Vec<i64> = sqlx::query_scalar(
            r"SELECT id FROM artists WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&get_state().await.db)
        .await
        .unwrap_or_default();
        for aid in artists {
            cleanup_artist(aid).await;
        }
        let subs: Vec<i64> = sqlx::query_scalar(
            r"SELECT id FROM mail_subscribers WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&get_state().await.db)
        .await
        .unwrap_or_default();
        for sid in subs {
            cleanup_subscriber(sid).await;
        }
        let _ = sqlx::query(r"DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&get_state().await.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mail_subscribers_index_authenticated() {
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
            .uri("/v1/mail_subscribers")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn mail_subscribers_index_forbidden() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let user_token = encode_token_with_role(99, TEST_SECRET, 3, Some("user".to_string())).unwrap();
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/mail_subscribers")
            .insert_header(("Authorization", format!("Bearer {}", user_token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_mailing_list_authenticated() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;
        let (sub_id, _) = seed_subscriber(artist_id, Some(user_id)).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/artist_mailing_list?artist_id={}", artist_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        let arr = body.as_array().unwrap();
        let ids: Vec<i64> = arr.iter().map(|v| v["id"].as_i64().unwrap()).collect();
        assert!(ids.contains(&sub_id));

        cleanup_subscriber(sub_id).await;
        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_mail_subscriber_public() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        let email = format!("addmail_{}_{}@test.com", pid, ts);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/addmailsubscriber")
            .set_json(serde_json::json!({
                "full_name": "Jane Doe",
                "email": email.clone()
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["email"], email);

        let _ = sqlx::query(r"DELETE FROM mail_subscribers WHERE email = $1")
            .bind(&email)
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_mail_subscriber_invalid_email() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

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
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        let email = format!("addmailuser_{}_{}@test.com", pid, ts);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/addmailsubscriber_with_user")
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

        let _ = sqlx::query(r"DELETE FROM mail_subscribers WHERE email = $1")
            .bind(&email)
            .execute(&state.db)
            .await;
        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_mail_subscriber_duplicate() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        let email = format!("dupmail_{}_{}@test.com", pid, ts);

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

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/artistmailsubscriber")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "full_name": "Jane Doe",
                "email": email.clone(),
                "artist_id": artist_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        let _ = sqlx::query(r"DELETE FROM mail_subscribers WHERE email = $1")
            .bind(&email)
            .execute(&state.db)
            .await;
        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unsubscribe_authenticated() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;
        let (sub_id, _) = seed_subscriber(artist_id, Some(user_id)).await;
        let user_token = encode_token_with_role(user_id, TEST_SECRET, 3, Some("admin".to_string())).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/mail_subscribers/unsubscribe")
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

        cleanup_subscriber(sub_id).await;
        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn unsubscribe_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let _max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM users",
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

        let req = test::TestRequest::post()
            .uri("/v1/mail_subscribers/unsubscribe")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "artist_id": 99999
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_unsubscribe_public() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

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

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/unsubscribe")
            .set_json(serde_json::json!({
                "email": email.clone()
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["token"].is_string());
        assert!(body["unsubscribe_url"].is_string());

        let _ = sqlx::query(r"DELETE FROM mail_subscribers WHERE email = $1")
            .bind(&email)
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn request_unsubscribe_not_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/unsubscribe")
            .set_json(serde_json::json!({
                "email": "nonexistent@test.com"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn process_unsubscribe_public() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

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

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/unsubscribe/{}", token))
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

        cleanup_subscriber(sub_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn process_unsubscribe_invalid_token() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/unsubscribe/invalidtoken123")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }
}
