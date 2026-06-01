//! User management handlers
//!
//! Provides CRUD endpoints for user accounts. Admins can manage all users;
//! regular users can only view and update their own profile.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/users` | admin | List all users |
//! | `show` | GET | `/v1/user/{id}` | auth | Retrieve a user (self or admin) |
//! | `create` | POST | `/v1/users` | public | Create a new user account |
//! | `update` | PUT | `/v1/user/{id}` | auth | Update a user (self or admin) |

use actix_web::{HttpResponse, web};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::sign_up_trigger::SignUpTrigger;
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
    token_version: i64,
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
            first_name: None,
            last_name: None,
            token_version: row.token_version,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

/// List all users.
///
/// Returns all users ordered by ID. Requires admin role.
///
/// # Response
///
/// `200 OK` — JSON array of `UserResponse`
/// `403 Forbidden` — non-admin user
pub async fn index(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let rows = sqlx::query_as::<_, UserRow>(
        r"SELECT id, email, password_digest, role, session_token, username, COALESCE(token_version, 1) as token_version, created_at, updated_at FROM users ORDER BY id"
    )
    .fetch_all(&state.db)
    .await?;

    let users: Vec<User> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<UserResponse> = users.iter().map(|u| u.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Retrieve a single user by ID.
///
/// Admins can view any user. Regular users can only view their own profile.
///
/// # Response
///
/// `200 OK` — `UserResponse`
/// `403 Forbidden` — non-admin viewing another user
/// `404 Not Found` — user does not exist
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
        r"SELECT id, email, password_digest, role, session_token, username, COALESCE(token_version, 1) as token_version, created_at, updated_at FROM users WHERE id = $1"
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

/// Create a new user account.
///
/// Validates email format, password strength, and optionally a sign-up token.
/// Email is normalized (trimmed, lowercased) before validation and storage.
/// When a token is provided, a database transaction with `SELECT ... FOR UPDATE`
/// locks the sign-up trigger row to prevent concurrent token reuse (TOCTOU).
/// Checks for existing user with the same normalized email before insertion.
/// The trigger is consumed (expired) upon successful user creation within the
/// same transaction. The role is always forced to "user" regardless of input.
/// No authentication required for public sign-up.
///
/// # Response
///
/// `201 Created` — `UserResponse` for the new user (email in normalized form)
/// `409 Conflict` — a user with this email already exists
/// `422 Unprocessable Entity` — validation error or invalid token
pub async fn create(
    state: web::Data<AppState>,
    body: web::Json<CreateUserRequest>,
) -> Result<HttpResponse, AppError> {
    let normalized_email = UserService::normalize_email(&body.email);

    if !User::validate_email(&normalized_email) {
        return Err(AppError::Validation("Invalid email address".to_string()));
    }
    if !User::validate_password(&body.password) {
        return Err(AppError::Validation(
            "Password must be at least 8 characters with uppercase, lowercase, and a digit".to_string(),
        ));
    }
    if body.token.is_none() || body.token.as_ref().unwrap().is_empty() {
        return Err(AppError::Validation(
            "Sign-up token is required".to_string(),
        ));
    }

    let mut tx = state.db.begin().await?;

    let token = body.token.as_ref().unwrap();
    let trigger = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        r"SELECT email, expires_at FROM sign_up_triggers WHERE token = $1 FOR UPDATE"
    )
    .bind(token)
    .fetch_optional(&mut *tx)
    .await?;

    match trigger {
        Some((trigger_email, trigger_expires)) => {
            if let Some(expires_str) = trigger_expires
                && let Some(expired_time) = SignUpTrigger::parse_expires_at(&expires_str)
                    && expired_time < chrono::Utc::now() {
                        return Err(AppError::Validation(
                            "Sign-up token has expired".to_string(),
                        ));
                    }

            let emails_match = trigger_email
                .as_ref()
                .map(|e| UserService::normalize_email(e))
                .as_deref()
                == Some(&normalized_email);

            if !emails_match {
                return Err(AppError::Validation(
                    "Email does not match sign-up token".to_string(),
                ));
            }
        }
        None => {
            return Err(AppError::Validation(
                "Invalid sign-up token".to_string(),
            ));
        }
    }

    let email_exists: bool = sqlx::query_scalar(
        r"SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)"
    )
    .bind(&normalized_email)
    .fetch_one(&mut *tx)
    .await?;

    if email_exists {
        return Err(AppError::Conflict(
            "A user with this email already exists".to_string(),
        ));
    }

    let password_digest = UserService::hash_password_for_create(&CreateUserRequest {
        email: normalized_email.clone(),
        password: body.password.clone(),
        username: body.username.clone(),
        token: body.token.clone(),
    })?;
    let role = UserService::force_role_user();
    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, UserRow>(
        r"INSERT INTO users (email, password_digest, role, session_token, username, token_version, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, 1, $6, $7)
           RETURNING id, email, password_digest, role, session_token, username, token_version, created_at, updated_at"
    )
    .bind(&normalized_email)
    .bind(&password_digest)
    .bind(&role)
    .bind::<Option<String>>(None)
    .bind(&body.username)
    .bind(now)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    let now_str = chrono::Utc::now().to_rfc3339();
    let _ = sqlx::query(
        r"UPDATE sign_up_triggers SET expires_at = $1, updated_at = $2 WHERE email = $3"
    )
    .bind(&now_str)
    .bind(now)
    .bind(&normalized_email)
    .execute(&mut *tx)
    .await;

    tx.commit().await?;

    let new_user: User = row.into();
    Ok(HttpResponse::Created().json(new_user.to_response()))
}

/// Update a user's profile.
///
/// Admins can update any user field. Regular users can only update their own
/// profile and cannot change their role. Password changes are hashed with bcrypt.
///
/// # Response
///
/// `200 OK` — updated `UserResponse`
/// `403 Forbidden` — non-admin viewing another user or attempting role change
/// `404 Not Found` — user does not exist
/// `422 Unprocessable Entity` — validation error
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
        && !user.is_admin()
    {
        return Err(AppError::Forbidden(
            "Only admins can change roles".to_string(),
        ));
    }

    let existing = sqlx::query_as::<_, UserRow>(
        r"SELECT id, email, password_digest, role, session_token, username, COALESCE(token_version, 1) as token_version, created_at, updated_at FROM users WHERE id = $1"
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
            let mut increment_token_version = false;

            if let Some(ref password) = body.password {
                password_digest =
                    Some(UserService::hash_password_for_create(&CreateUserRequest {
                        email: row.email.clone(),
                        password: password.clone(),
                        username: None,
                        token: None,
                    })?);
                increment_token_version = true;
            }

            let now = chrono::Utc::now().naive_utc();

            let updated_row = sqlx::query_as::<_, UserRow>(
                r"UPDATE users
                   SET email = COALESCE($1, email),
                       username = COALESCE($2, username),
                       role = COALESCE($3, role),
                       password_digest = COALESCE($4, password_digest),
                       token_version = CASE WHEN $7 THEN COALESCE(token_version, 1) + 1 ELSE token_version END,
                       updated_at = $5
                   WHERE id = $6
                   RETURNING id, email, password_digest, role, session_token, username, COALESCE(token_version, 1) as token_version, created_at, updated_at"
            )
            .bind(email)
            .bind(username)
            .bind(role)
            .bind(password_digest)
            .bind(now)
            .bind(target_id)
            .bind(increment_token_version)
            .fetch_one(&state.db)
            .await?;

            let u: User = updated_row.into();
            Ok(HttpResponse::Ok().json(u.to_response()))
        }
        None => Err(AppError::NotFound(format!("User #{}", target_id))),
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/users", web::get().to(index))
        .route("/user/{id}", web::get().to(show))
        .route("/users", web::post().to(create))
        .route("/user/{id}", web::put().to(update));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token_with_role;
    use actix_web::{App, test};

    const TEST_SECRET: &str = "test-secret-key";

    fn setup_env() {
        unsafe {
            std::env::set_var("DATABASE_URL", &crate::test_utils::test_db_url());
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
    }

    async fn get_state() -> AppState {
        let pool = sqlx::PgPool::connect(crate::test_utils::test_db_url())
            .await
            .expect("Failed to connect to test database");
        crate::test_utils::build_test_state(pool).await
    }

    fn admin_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap()
    }

    fn user_token(user_id: i64) -> String {
        encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap()
    }

    async fn seed_test_user(state: &AppState, email: &str, role: &str) -> i64 {
        let password_digest = UserService::hash_password_for_create(&CreateUserRequest {
            email: email.to_string(),
            password: "Password123".to_string(),
            username: None,
            token: None,
        })
        .unwrap();
        let now = chrono::Utc::now().naive_utc();
        let row = sqlx::query_as::<_, UserRow>(
            r"INSERT INTO users (email, password_digest, role, session_token, username, token_version, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, 1, $6, $7)
               RETURNING id, email, password_digest, role, session_token, username, token_version, created_at, updated_at"
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

    async fn seed_trigger(state: &AppState, email: &str, token: &str, expires_at: &str) -> i64 {
        let now = chrono::Utc::now().naive_utc();
        sqlx::query_scalar(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ($1, $2, $3, 'user', $4, $4) RETURNING id"
        )
        .bind(email)
        .bind(token)
        .bind(expires_at)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed trigger")
    }

    async fn cleanup_trigger(state: &AppState, email: &str) {
        let _ = sqlx::query(r"DELETE FROM sign_up_triggers WHERE email = $1")
            .bind(email)
            .execute(&state.db)
            .await;
    }

    // — index —

    #[tokio::test(flavor = "current_thread")]
    async fn user_index_non_admin_forbidden() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let app = test::init_service(App::new().app_data(state).configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/users")
            .insert_header(("Authorization", format!("Bearer {}", user_token(2))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    // — show —

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_self() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email = format!("showself{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;
        let token =
            encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/user/{}", user_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_other_not_admin() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email_a = format!("showa{}@example.com", ts);
        let email_b = format!("showb{}@example.com", ts);
        let other_id = seed_test_user(&state, &email_a, "user").await;
        let viewer_id = seed_test_user(&state, &email_b, "user").await;
        let viewer_token =
            encode_token_with_role(viewer_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/user/{}", other_id))
            .insert_header(("Authorization", format!("Bearer {}", viewer_token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        cleanup_user(&state, &email_a).await;
        cleanup_user(&state, &email_b).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_admin_sees_other() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email = format!("showadmin{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/user/{}", user_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_not_found() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let app =
            test::init_service(App::new().app_data(state.clone()).configure(config_routes)).await;

        let max_id: i64 = sqlx::query_scalar(r"SELECT COALESCE(MAX(id), 0) FROM users")
            .fetch_one(&state.db)
            .await
            .expect("Failed to get max id");

        let req = test::TestRequest::get()
            .uri(&format!("/user/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    // — create —

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_success() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email = format!("newuser{}@example.com", ts);
        let username = format!("newuser{}", ts);
        let token = format!("create_success_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        seed_trigger(&state, &email, &token, &future).await;

        let app =
            test::init_service(App::new().app_data(state.clone()).configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/users")
            .set_json(serde_json::json!({
                "email": email,
                "password": "Password123",
                "username": username,
                "token": token
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["role"], "user");

        cleanup_user(&state, &email).await;
        cleanup_trigger(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_with_valid_token_consumes_trigger() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email = format!("tokenuser{}@example.com", ts);
        let username = format!("tokenuser{}", ts);
        let token = format!("valid_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        let trigger_id = seed_trigger(&state, &email, &token, &future).await;

        let app =
            test::init_service(App::new().app_data(state.clone()).configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/users")
            .set_json(serde_json::json!({
                "email": email,
                "password": "Password123",
                "username": username,
                "token": token
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let expired: Option<String> = sqlx::query_scalar(
            r"SELECT expires_at FROM sign_up_triggers WHERE id = $1"
        )
        .bind(trigger_id)
        .fetch_one(&state.db)
        .await
        .expect("Failed to check trigger");

        assert!(
            crate::models::sign_up_trigger::SignUpTrigger::parse_expires_at_for_test(
                expired.as_deref()
            )
            .unwrap()
                < chrono::Utc::now(),
            "Trigger should be consumed"
        );

        cleanup_user(&state, &email).await;
        cleanup_trigger(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_with_expired_token_fails() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email = format!("expiredtoken{}@example.com", ts);
        let token = format!("expired_token_{}", ts);
        let past = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        seed_trigger(&state, &email, &token, &past).await;

        let app =
            test::init_service(App::new().app_data(state.clone()).configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/users")
            .set_json(serde_json::json!({
                "email": email,
                "password": "Password123",
                "username": "expireduser",
                "token": token
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        cleanup_user(&state, &email).await;
        cleanup_trigger(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_with_mismatched_email_token_fails() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let trigger_email = format!("trigger_email_{}@example.com", ts);
        let request_email = format!("request_email_{}@example.com", ts);
        let token = format!("mismatch_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        seed_trigger(&state, &trigger_email, &token, &future).await;

        let app =
            test::init_service(App::new().app_data(state.clone()).configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/users")
            .set_json(serde_json::json!({
                "email": request_email,
                "password": "Password123",
                "username": "mismatchuser",
                "token": token
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        cleanup_user(&state, &trigger_email).await;
        cleanup_user(&state, &request_email).await;
        cleanup_trigger(&state, &trigger_email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_without_token_fails() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let app = test::init_service(App::new().app_data(state).configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/users")
            .set_json(serde_json::json!({
                "email": "notoken@example.com",
                "password": "Password123"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_duplicate_email_returns_409() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email = format!("dupemail{}@example.com", ts);
        let username2 = format!("dupuser{}b", ts);

        seed_test_user(&state, &email, "user").await;

        let token2 = format!("dup_token2_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        seed_trigger(&state, &email, &token2, &future).await;

        let app =
            test::init_service(App::new().app_data(state.clone()).configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/users")
            .set_json(serde_json::json!({
                "email": email.clone(),
                "password": "Password456",
                "username": username2,
                "token": token2
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 409);

        cleanup_user(&state, &email).await;
        cleanup_trigger(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_email_normalized_on_create() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email_upper = format!("Normalised{}@Example.COM", ts);
        let email_lower = format!("normalised{}@example.com", ts);

        let token = format!("norm_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        seed_trigger(&state, &email_lower, &token, &future).await;

        let app =
            test::init_service(App::new().app_data(state.clone()).configure(config_routes)).await;

        // Creation with mixed-case email succeeds and returns normalized lowercase
        let req = test::TestRequest::post()
            .uri("/users")
            .set_json(serde_json::json!({
                "email": email_upper,
                "password": "Password123",
                "token": token
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["email"], email_lower);

        cleanup_user(&state, &email_lower).await;
        cleanup_trigger(&state, &email_lower).await;
    }

    // — update —

    #[tokio::test(flavor = "current_thread")]
    async fn user_update_self() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email = format!("updateself{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;
        let token =
            encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/user/{}", user_id))
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
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email_a = format!("updatea{}@example.com", ts);
        let email_b = format!("updateb{}@example.com", ts);
        let other_id = seed_test_user(&state, &email_a, "user").await;
        let viewer_id = seed_test_user(&state, &email_b, "user").await;
        let viewer_token =
            encode_token_with_role(viewer_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/user/{}", other_id))
            .insert_header(("Authorization", format!("Bearer {}", viewer_token)))
            .set_json(serde_json::json!({
                "username": "hacked"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        cleanup_user(&state, &email_a).await;
        cleanup_user(&state, &email_b).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_update_admin_changes_other() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email = format!("updateadmin{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/user/{}", user_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "username": "adminupdated"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["username"], "adminupdated");

        cleanup_user(&state, &email).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_update_non_admin_cannot_change_role() {
        setup_env();
        let state = web::Data::new(get_state().await);
        let ts = timestamp();
        let email = format!("updaterole{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;
        let token =
            encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/user/{}", user_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "role": "admin"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        cleanup_user(&state, &email).await;
    }

    // — cross-handler —

    #[tokio::test(flavor = "current_thread")]
    async fn password_change_revokes_old_tokens() {
        setup_env();
        let state_data = web::Data::new(get_state().await);
        let ts = timestamp();
        let email = format!("pwdchange{}@example.com", ts);
        let user_id = seed_test_user(&state_data, &email, "user").await;
        let old_token = encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let app = test::init_service(
            App::new()
                .app_data(state_data.clone())
                .app_data(web::Data::new(
                    state_data.cookie_builder.clone()
                ))
                .configure(crate::handlers::auth::config_routes)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/user/{}", user_id))
            .insert_header(("Authorization", format!("Bearer {}", old_token)))
            .set_json(serde_json::json!({
                "password": "NewPassword123"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let req = test::TestRequest::get()
            .uri("/session")
            .insert_header(("Authorization", format!("Bearer {}", old_token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);

        cleanup_user(&state_data, &email).await;
    }

    fn timestamp() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }
}
