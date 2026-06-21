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

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::data::users as data;
use crate::error::AppError;
use crate::models::user::{CreateUserRequest, UpdateUserRequest, User, UserResponse};
use crate::services::user_service::UserService;

#[cfg(test)]
use crate::models::user::UserRow;

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
    let users = data::list(&state.db).await?;
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
    match data::by_id(&state.db, target_id).await? {
        Some(u) => Ok(HttpResponse::Ok().json(u.to_response())),
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
            "Password must be at least 8 characters with uppercase, lowercase, and a digit"
                .to_string(),
        ));
    }
    if body.token.is_none() || body.token.as_ref().unwrap().is_empty() {
        return Err(AppError::Validation(
            "Sign-up token is required".to_string(),
        ));
    }

    let new_user = UserService::create_user_with_trigger(&state.db, &body).await?;
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

    let existing = data::by_id(&state.db, target_id).await?;

    match existing {
        Some(existing_user) => {
            let password_digest = body
                .password
                .as_ref()
                .map(|pwd| {
                    UserService::hash_password_for_create(&CreateUserRequest {
                        email: existing_user.email.clone(),
                        password: pwd.clone(),
                        username: None,
                        token: None,
                    })
                })
                .transpose()?;

            let now = chrono::Utc::now().naive_utc();

            let updated = data::update_with_password(
                &state.db,
                target_id,
                &body.email,
                &body.username,
                &body.role,
                &password_digest,
                password_digest.is_some(),
                now,
            )
            .await?;

            match updated {
                Some(u) => Ok(HttpResponse::Ok().json(u.to_response())),
                None => Err(AppError::NotFound(format!("User #{}", target_id))),
            }
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
     use crate::test_utils::{admin_token, not_found_id, user_token, unique_suffix, TEST_SECRET};
    use actix_web::test;

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

    async fn seed_trigger(state: &AppState, email: &str, token: &str, expires_at: &str) -> i64 {
        let now = chrono::Utc::now().naive_utc();
        sqlx::query_scalar(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ($1, $2, $3, 'user', $4, $4) RETURNING id",
        )
        .bind(email)
        .bind(token)
        .bind(expires_at)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed trigger")
    }

    // — index —

    #[tokio::test(flavor = "current_thread")]
    async fn user_index_non_admin_forbidden() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/users")
            .insert_header(("Authorization", format!("Bearer {}", user_token(2))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    // — show —

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_self() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email = format!("showself{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;
        let token =
            encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let req = test::TestRequest::get()
            .uri(&format!("/user/{}", user_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_other_not_admin() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email_a = format!("showa{}@example.com", ts);
        let email_b = format!("showb{}@example.com", ts);
        let other_id = seed_test_user(&state, &email_a, "user").await;
        let viewer_id = seed_test_user(&state, &email_b, "user").await;
        let viewer_token =
            encode_token_with_role(viewer_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let req = test::TestRequest::get()
            .uri(&format!("/user/{}", other_id))
            .insert_header(("Authorization", format!("Bearer {}", viewer_token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_admin_sees_other() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email = format!("showadmin{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;

        let req = test::TestRequest::get()
            .uri(&format!("/user/{}", user_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "users").await;

        let req = test::TestRequest::get()
            .uri(&format!("/user/{}", not_found))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    // — create —

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_success() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email = format!("newuser{}@example.com", ts);
        let username = format!("newuser{}", ts);
        let token = format!("create_success_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        seed_trigger(&state, &email, &token, &future).await;

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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_with_valid_token_consumes_trigger() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email = format!("tokenuser{}@example.com", ts);
        let username = format!("tokenuser{}", ts);
        let token = format!("valid_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        let trigger_id = seed_trigger(&state, &email, &token, &future).await;

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

        let expired: Option<String> =
            sqlx::query_scalar(r"SELECT expires_at FROM sign_up_triggers WHERE id = $1")
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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_with_expired_token_fails() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email = format!("expiredtoken{}@example.com", ts);
        let token = format!("expired_token_{}", ts);
        let past = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        seed_trigger(&state, &email, &token, &past).await;

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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_with_mismatched_email_token_fails() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let trigger_email = format!("trigger_email_{}@example.com", ts);
        let request_email = format!("request_email_{}@example.com", ts);
        let token = format!("mismatch_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        seed_trigger(&state, &trigger_email, &token, &future).await;

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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_without_token_fails() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/users")
            .set_json(serde_json::json!({
                "email": "notoken@example.com",
                "password": "Password123"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_duplicate_email_returns_409() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email = format!("dupemail{}@example.com", ts);
        let username2 = format!("dupuser{}b", ts);

        seed_test_user(&state, &email, "user").await;

        let token2 = format!("dup_token2_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        seed_trigger(&state, &email, &token2, &future).await;

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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_email_normalized_on_create() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email_upper = format!("Normalised{}@Example.COM", ts);
        let email_lower = format!("normalised{}@example.com", ts);

        let token = format!("norm_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        seed_trigger(&state, &email_lower, &token, &future).await;

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

        _guard.cleanup().await;
    }

    // — update —

    #[tokio::test(flavor = "current_thread")]
    async fn user_update_self() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email = format!("updateself{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;
        let token =
            encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_update_other_not_admin() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email_a = format!("updatea{}@example.com", ts);
        let email_b = format!("updateb{}@example.com", ts);
        let other_id = seed_test_user(&state, &email_a, "user").await;
        let viewer_id = seed_test_user(&state, &email_b, "user").await;
        let viewer_token =
            encode_token_with_role(viewer_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let req = test::TestRequest::put()
            .uri(&format!("/user/{}", other_id))
            .insert_header(("Authorization", format!("Bearer {}", viewer_token)))
            .set_json(serde_json::json!({
                "username": "hacked"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_update_admin_changes_other() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email = format!("updateadmin{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;

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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_update_non_admin_cannot_change_role() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = unique_suffix();
        let email = format!("updaterole{}@example.com", ts);
        let user_id = seed_test_user(&state, &email, "user").await;
        let token =
            encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let req = test::TestRequest::put()
            .uri(&format!("/user/{}", user_id))
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "role": "admin"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    // — cross-handler —

    #[tokio::test(flavor = "current_thread")]
    async fn password_change_revokes_old_tokens() {
        crate::test_utils::set_test_env_jwt();
        fn auth_and_users_routes(cfg: &mut actix_web::web::ServiceConfig) {
            cfg.configure(crate::handlers::auth::config_routes)
               .configure(config_routes);
        }
        let (_guard, state_data, app) = crate::build_test_app_with_cookies!(auth_and_users_routes);
        let ts = unique_suffix();
        let email = format!("pwdchange{}@example.com", ts);
        let user_id = seed_test_user(&state_data, &email, "user").await;
        let old_token =
            encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

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

        _guard.cleanup().await;
    }
}
