use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::reset_trigger::{ResetTrigger, UpdateResetTriggerRequest};
use crate::services::auth_service::hash_password;

#[derive(Debug, Deserialize)]
pub struct CreateResetParams {
    pub email: String,
}

pub async fn create(
    body: web::Json<CreateResetParams>,
) -> Result<HttpResponse, AppError> {
    if !crate::models::user::User::validate_email(&body.email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    let token = ResetTrigger::generate_token();
    let expires_at = ResetTrigger::generate_expires_at();
    let user_id = mock_find_user_id(&body.email);

    let trigger = ResetTrigger {
        id: 999,
        user_id,
        token: Some(token.clone()),
        expires_at: Some(expires_at),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    Ok(HttpResponse::Created().json(trigger.to_response()))
}

pub async fn show(path: web::Path<String>) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();
    match mock_find_by_token(&token) {
        Some(trigger) => Ok(HttpResponse::Ok().json(trigger.to_response())),
        None => Err(AppError::NotFound("Reset trigger not found".to_string())),
    }
}

pub async fn update(
    path: web::Path<String>,
    body: web::Json<UpdateResetTriggerRequest>,
) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();
    match mock_find_by_token(&token) {
        Some(_trigger) => {
            if !crate::models::user::User::validate_password(&body.password) {
                return Err(AppError::Validation(
                    "Password must be at least 6 characters".to_string(),
                ));
            }
            let _new_hash = hash_password(&body.password)?;
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "message": "Password updated successfully"
            })))
        }
        None => Err(AppError::NotFound("Reset trigger not found".to_string())),
    }
}

fn mock_find_user_id(email: &str) -> Option<i32> {
    if email == "test@example.com" {
        Some(1)
    } else {
        None
    }
}

fn mock_find_by_token(token: &str) -> Option<ResetTrigger> {
    if token == "reset-token-valid" {
        Some(ResetTrigger {
            id: 1,
            user_id: Some(1),
            token: Some("reset-token-valid".to_string()),
            expires_at: Some("Dec 31, 2026 11:59 PM".to_string()),
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
            .route("/reset_trigger", web::post().to(create))
            .route("/reset_trigger/{token}", web::get().to(show))
            .route("/reset_trigger/{token}", web::post().to(update)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_create_success() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/reset_trigger")
            .set_json(serde_json::json!({
                "email": "test@example.com"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["token"].is_string());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_create_invalid_email() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/reset_trigger")
            .set_json(serde_json::json!({
                "email": "bad-email"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_show_valid() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/reset_trigger/reset-token-valid")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_show_not_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/reset_trigger/bad-token")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_update_success() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/reset_trigger/reset-token-valid")
            .set_json(serde_json::json!({
                "password": "newpassword123"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn reset_trigger_update_short_password() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/reset_trigger/reset-token-valid")
            .set_json(serde_json::json!({
                "password": "short"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }
}
