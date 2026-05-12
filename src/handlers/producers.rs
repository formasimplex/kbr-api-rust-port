use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::producer::{CreateProducerRequest, Producer, ProducerResponse, UpdateProducerRequest};

pub async fn index(_user: CurrentUser) -> Result<HttpResponse, AppError> {
    let producers = mock_all_producers();
    let responses: Vec<ProducerResponse> = producers.iter().map(|p| p.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn create(
    _user: CurrentUser,
    body: web::Json<CreateProducerRequest>,
) -> Result<HttpResponse, AppError> {
    if body.producer_name.is_empty() {
        return Err(AppError::Validation("producer_name is required".to_string()));
    }
    let producer = Producer {
        id: 999,
        description: body.description.clone(),
        producer_name: body.producer_name.clone(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(producer.to_response()))
}

pub async fn update(
    _user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateProducerRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_producer(id) {
        Some(mut p) => {
            if let Some(ref name) = body.producer_name {
                if name.is_empty() {
                    return Err(AppError::Validation("producer_name cannot be empty".to_string()));
                }
                p.producer_name = name.clone();
            }
            if let Some(ref desc) = body.description {
                p.description = Some(desc.clone());
            }
            Ok(HttpResponse::Ok().json(p.to_response()))
        }
        None => Err(AppError::NotFound(format!("Producer #{}", id))),
    }
}

fn mock_all_producers() -> Vec<Producer> {
    vec![Producer {
        id: 1,
        description: Some("Vinyl presser".to_string()),
        producer_name: "Test Pressing".to_string(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_find_producer(id: i64) -> Option<Producer> {
    if id == 1 {
        Some(Producer {
            id: 1,
            description: Some("Vinyl presser".to_string()),
            producer_name: "Test Pressing".to_string(),
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
            .route("/producers", web::get().to(index))
            .route("/producers", web::post().to(create))
            .route("/producers/{id}", web::put().to(update)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";

    fn token() -> String {
        crate::auth::jwt::encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producers_index_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/producers")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producer_create_success() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/producers")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .set_json(serde_json::json!({
                "producer_name": "New Pressing Co",
                "description": "Premium vinyl"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producer_create_empty_name() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/producers")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .set_json(serde_json::json!({
                "producer_name": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }
}
