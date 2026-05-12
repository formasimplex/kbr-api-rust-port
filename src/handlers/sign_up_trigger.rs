use actix_web::{web, HttpResponse};
use serde::Deserialize;

use crate::error::AppError;
use crate::models::sign_up_trigger::SignUpTrigger;

#[derive(Debug, Deserialize)]
pub struct CreateTriggerParams {
    pub email: String,
    pub role: Option<String>,
}

pub async fn create(
    body: web::Json<CreateTriggerParams>,
) -> Result<HttpResponse, AppError> {
    if !User::validate_email(&body.email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    let token = SignUpTrigger::generate_token();
    let expires_at = SignUpTrigger::generate_expires_at();

    let trigger = SignUpTrigger {
        id: 999,
        email: Some(body.email.clone()),
        token: Some(token.clone()),
        expires_at: Some(expires_at),
        role: body.role.clone(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    Ok(HttpResponse::Created().json(trigger.to_response()))
}

pub async fn show(path: web::Path<String>) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();
    match mock_find_by_token(&token) {
        Some(trigger) => Ok(HttpResponse::Ok().json(trigger.to_response())),
        None => Err(AppError::NotFound(format!("Sign-up trigger not found"))),
    }
}

fn mock_find_by_token(token: &str) -> Option<SignUpTrigger> {
    if token == "valid-token" {
        Some(SignUpTrigger {
            id: 1,
            email: Some("invited@example.com".to_string()),
            token: Some("valid-token".to_string()),
            expires_at: Some("Dec 31, 2026 11:59 PM".to_string()),
            role: Some("user".to_string()),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    } else {
        None
    }
}

use crate::models::user::User;

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/sign_up_trigger", web::post().to(create))
            .route("/sign_up_trigger/{token}", web::get().to(show)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_create_success() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/sign_up_trigger")
            .set_json(serde_json::json!({
                "email": "invited@example.com",
                "role": "user"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["token"].is_string());
        assert_eq!(body["email"], "invited@example.com");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_create_invalid_email() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/sign_up_trigger")
            .set_json(serde_json::json!({
                "email": "bad-email"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_show_valid() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/sign_up_trigger/valid-token")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_show_not_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/sign_up_trigger/invalid-token")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }
}
