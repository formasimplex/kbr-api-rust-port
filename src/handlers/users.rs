use actix_web::{web, HttpResponse};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::user::{CreateUserRequest, UpdateUserRequest, User, UserResponse};
use crate::services::user_service::UserService;

#[derive(Debug, FromRow)]
struct UserRow {
    id: i64,
    email: String,
    password_digest: String,
    role: Option<String>,
    session_token: Option<String>,
    username: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        User {
            id: row.id,
            email: row.email,
            password_digest: row.password_digest,
            role: row.role,
            session_token: row.session_token,
            username: row.username,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

pub async fn index(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let rows = sqlx::query_as::<_, UserRow>(
        r"SELECT id, email, password_digest, role, session_token, username, created_at, updated_at FROM users ORDER BY id"
    )
    .fetch_all(&state.db)
    .await?;

    let users: Vec<User> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<UserResponse> = users.iter().map(|u| u.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    if !user.is_admin() && user.id != target_id {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    match sqlx::query_as::<_, UserRow>(
        r"SELECT id, email, password_digest, role, session_token, username, created_at, updated_at FROM users WHERE id = $1"
    )
    .bind(target_id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let u: User = row.into();
            Ok(HttpResponse::Ok().json(u.to_response()))
        }
        None => Err(AppError::NotFound(format!("User #{}", target_id))),
    }
}

pub async fn create(
    state: web::Data<AppState>,
    body: web::Json<CreateUserRequest>,
) -> Result<HttpResponse, AppError> {
    UserService::validate_create_request(&body)?;

    if let Some(ref token) = body.token
        && token.is_empty() {
            return Err(AppError::Validation(
                "Sign-up token is required".to_string(),
            ));
        }

    let password_digest = UserService::hash_password_for_create(&body)?;
    let role = UserService::force_role_user();
    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, UserRow>(
        r"INSERT INTO users (email, password_digest, role, session_token, username, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, email, password_digest, role, session_token, username, created_at, updated_at"
    )
    .bind(&body.email)
    .bind(&password_digest)
    .bind(&role)
    .bind::<Option<String>>(None)
    .bind(&body.username)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let new_user: User = row.into();
    Ok(HttpResponse::Created().json(new_user.to_response()))
}

pub async fn update(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateUserRequest>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    if !user.is_admin() && user.id != target_id {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    UserService::validate_update_request(&body)?;

    if let Some(ref _role) = body.role
        && !user.is_admin() {
            return Err(AppError::Forbidden(
                "Only admins can change roles".to_string(),
            ));
        }

    let existing = sqlx::query_as::<_, UserRow>(
        r"SELECT id, email, password_digest, role, session_token, username, created_at, updated_at FROM users WHERE id = $1"
    )
    .bind(target_id)
    .fetch_optional(&state.db)
    .await?;

    match existing {
        Some(row) => {
            let email = body.email.clone();
            let username = body.username.clone();
            let role = body.role.clone();
            let mut password_digest: Option<String> = None;

            if let Some(ref password) = body.password {
                password_digest = Some(UserService::hash_password_for_create(&CreateUserRequest {
                    email: row.email.clone(),
                    password: password.clone(),
                    username: None,
                    token: None,
                })?);
            }

            let now = chrono::Utc::now().naive_utc();

            let updated_row = sqlx::query_as::<_, UserRow>(
                r"UPDATE users
                   SET email = COALESCE($1, email),
                       username = COALESCE($2, username),
                       role = COALESCE($3, role),
                       password_digest = COALESCE($4, password_digest),
                       updated_at = $5
                   WHERE id = $6
                   RETURNING id, email, password_digest, role, session_token, username, created_at, updated_at"
            )
            .bind(email)
            .bind(username)
            .bind(role)
            .bind(password_digest)
            .bind(now)
            .bind(target_id)
            .fetch_one(&state.db)
            .await?;

            let u: User = updated_row.into();
            Ok(HttpResponse::Ok().json(u.to_response()))
        }
        None => Err(AppError::NotFound(format!("User #{}", target_id))),
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/users", web::get().to(index))
            .route("/user/{id}", web::get().to(show))
            .route("/users", web::post().to(create))
            .route("/user/{id}", web::put().to(update)),
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

    fn user_token(user_id: i64) -> String {
        encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string())).unwrap()
    }

    async fn seed_test_user(state: &AppState, email: &str, role: &str) -> i64 {
        let password_digest = UserService::hash_password_for_create(&CreateUserRequest {
            email: email.to_string(),
            password: "password123".to_string(),
            username: None,
            token: None,
        }).unwrap();
        let now = chrono::Utc::now().naive_utc();
        let row = sqlx::query_as::<_, UserRow>(
            r"INSERT INTO users (email, password_digest, role, session_token, username, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7)
               RETURNING id, email, password_digest, role, session_token, username, created_at, updated_at"
        )
        .bind(email)
        .bind(&password_digest)
        .bind(role)
        .bind::<Option<String>>(None)
        .bind::<Option<String>>(None)
        .bind(&now)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed test user");
        row.id
    }

    async fn cleanup_user(state: &AppState, email: &str) {
        let _ = sqlx::query(r"DELETE FROM users WHERE email = $1")
            .bind(email)
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_index_admin_sees_all() {
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
            .uri("/v1/users")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_index_non_admin_forbidden() {
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
            .uri("/v1/users")
            .insert_header(("Authorization", format!("Bearer {}", user_token(2))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_self() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("showself{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;
        let token = encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string())).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/user/{}", user_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_other_not_admin() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("showother{}@example.com", ts);
        let _other_id = seed_test_user(&state, &email, "user").await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/user/99999")
            .insert_header(("Authorization", format!("Bearer {}", user_token(2))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_not_found() {
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

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM users"
        )
        .fetch_one(&state.db)
        .await
        .expect("Failed to get max id");

        let req = test::TestRequest::get()
            .uri(&format!("/v1/user/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_success() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("newuser{}@example.com", ts);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/users")
            .set_json(serde_json::json!({
                "email": email,
                "password": "password123",
                "username": "newuser",
                "token": "valid-trigger-token"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["role"], "user");

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_invalid_email() {
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
            .uri("/v1/users")
            .set_json(serde_json::json!({
                "email": "bad-email",
                "password": "password123"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_short_password() {
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
            .uri("/v1/users")
            .set_json(serde_json::json!({
                "email": "new@example.com",
                "password": "short"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_update_self() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("updateself{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;
        let token = encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string())).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/v1/user/{}", user_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "username": "updateduser"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["username"], "updateduser");

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_update_other_not_admin() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let email = format!("updateother{}@example.com", ts);
        let _other_id = seed_test_user(&state, &email, "user").await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri("/v1/user/99999")
            .insert_header(("Authorization", format!("Bearer {}", user_token(2))))
            .set_json(serde_json::json!({
                "username": "hacked"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        cleanup_user(&state, &email).await;
    }
}
