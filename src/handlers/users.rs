use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::user::{CreateUserRequest, UpdateUserRequest, User, UserResponse};
use crate::services::user_service::UserService;

pub async fn index(user: CurrentUser) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let users = mock_all_users();
    let responses: Vec<UserResponse> = users.iter().map(|u| u.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    if !user.is_admin() && user.id != target_id {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    match mock_find_user(target_id) {
        Some(u) => Ok(HttpResponse::Ok().json(u.to_response())),
        None => Err(AppError::NotFound(format!("User #{}", target_id))),
    }
}

pub async fn create(
    body: web::Json<CreateUserRequest>,
) -> Result<HttpResponse, AppError> {
    UserService::validate_create_request(&body)?;

    if let Some(ref token) = body.token {
        if token.is_empty() {
            return Err(AppError::Validation(
                "Sign-up token is required".to_string(),
            ));
        }
    }

    let new_user = User {
        id: 999,
        email: body.email.clone(),
        password_digest: UserService::hash_password_for_create(&body)?,
        role: Some(UserService::force_role_user()),
        session_token: None,
        username: body.username.clone(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    Ok(HttpResponse::Created().json(new_user.to_response()))
}

pub async fn update(
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateUserRequest>,
) -> Result<HttpResponse, AppError> {
    let target_id = path.into_inner();
    if !user.is_admin() && user.id != target_id {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    UserService::validate_update_request(&body)?;
    match mock_find_user(target_id) {
        Some(mut u) => {
            if let Some(ref email) = body.email {
                u.email = email.clone();
            }
            if let Some(ref username) = body.username {
                u.username = Some(username.clone());
            }
            if let Some(ref role) = body.role {
                if !user.is_admin() {
                    return Err(AppError::Forbidden(
                        "Only admins can change roles".to_string(),
                    ));
                }
                u.role = Some(role.clone());
            }
            if let Some(ref password) = body.password {
                u.password_digest = UserService::hash_password_for_create(&CreateUserRequest {
                    email: u.email.clone(),
                    password: password.clone(),
                    username: None,
                    token: None,
                })?;
            }
            Ok(HttpResponse::Ok().json(u.to_response()))
        }
        None => Err(AppError::NotFound(format!("User #{}", target_id))),
    }
}

fn mock_all_users() -> Vec<User> {
    vec![
        User {
            id: 1,
            email: "admin@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("admin".to_string()),
            session_token: None,
            username: Some("admin".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        User {
            id: 2,
            email: "user@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("user".to_string()),
            session_token: None,
            username: Some("user".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
    ]
}

fn mock_find_user(id: i64) -> Option<User> {
    match id {
        1 => Some(User {
            id: 1,
            email: "admin@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("admin".to_string()),
            session_token: None,
            username: Some("admin".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }),
        2 => Some(User {
            id: 2,
            email: "user@example.com".to_string(),
            password_digest: "hashed".to_string(),
            role: Some("user".to_string()),
            session_token: None,
            username: Some("user".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }),
        _ => None,
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
    use crate::auth::jwt::encode_token;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";

    fn admin_token() -> String {
        crate::auth::jwt::encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    fn user_token() -> String {
        encode_token(2, TEST_SECRET, 3).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_index_admin_sees_all() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/users")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_index_non_admin_forbidden() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/users")
            .insert_header(("Authorization", format!("Bearer {}", user_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_self() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/user/2")
            .insert_header(("Authorization", format!("Bearer {}", user_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_other_not_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/user/1")
            .insert_header(("Authorization", format!("Bearer {}", user_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_show_not_found() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/user/9999")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_success() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/users")
            .set_json(serde_json::json!({
                "email": "new@example.com",
                "password": "password123",
                "username": "newuser",
                "token": "valid-trigger-token"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["role"], "user");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_create_invalid_email() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

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
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

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
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::put()
            .uri("/v1/user/2")
            .insert_header(("Authorization", format!("Bearer {}", user_token())))
            .set_json(serde_json::json!({
                "username": "updateduser"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn user_update_other_not_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::put()
            .uri("/v1/user/1")
            .insert_header(("Authorization", format!("Bearer {}", user_token())))
            .set_json(serde_json::json!({
                "username": "hacked"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }
}
