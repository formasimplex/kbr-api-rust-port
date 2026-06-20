//! Producer handlers
//!
//! Provides CRUD endpoints for record producer/pressing plant management.
//! All endpoints require authentication.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/producers` | auth | List all producers |
//! | `create` | POST | `/v1/producers` | auth | Create a new producer |
//! | `update` | PUT | `/v1/producers/{id}` | auth | Update producer details |

use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::data::producers as data;
use crate::error::AppError;
use crate::models::producer::{CreateProducerRequest, ProducerResponse, UpdateProducerRequest};

/// List all producers.
///
/// Returns all producers ordered by ID. Requires authentication.
///
/// # Response
///
/// `200 OK` — JSON array of `ProducerResponse`
pub async fn index(
    _user: CurrentUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let producers = data::list(&state.db).await?;
    let responses: Vec<ProducerResponse> = producers.iter().map(|p| p.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Create a new producer.
///
/// Validates that producer_name is non-empty. Requires authentication.
///
/// # Response
///
/// `201 Created` — `ProducerResponse` for the new producer
/// `422 Unprocessable Entity` — empty producer_name
pub async fn create(
    state: web::Data<AppState>,
    _user: CurrentUser,
    body: web::Json<CreateProducerRequest>,
) -> Result<HttpResponse, AppError> {
    if body.producer_name.is_empty() {
        return Err(AppError::Validation("producer_name is required".to_string()));
    }

    let now = chrono::Utc::now().naive_utc();

    let producer = data::create(&state.db, &body.producer_name, body.description.as_deref(), now).await?;
    Ok(HttpResponse::Created().json(producer.to_response()))
}

/// Update producer details.
///
/// Performs a partial update using COALESCE — only provided fields are changed.
/// Validates that producer_name is non-empty if provided. Requires authentication.
///
/// # Response
///
/// `200 OK` — updated `ProducerResponse`
/// `404 Not Found` — producer does not exist
/// `422 Unprocessable Entity` — empty producer_name
pub async fn update(
    state: web::Data<AppState>,
    _user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateProducerRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    let name = body.producer_name.clone();
    let desc = body.description.clone();

    if let Some(ref n) = body.producer_name
        && n.is_empty() {
            return Err(AppError::Validation("producer_name cannot be empty".to_string()));
        }

    let now = chrono::Utc::now().naive_utc();

    match data::update(&state.db, id, name.as_deref(), desc.as_deref(), now).await? {
        Some(producer) => Ok(HttpResponse::Ok().json(producer.to_response())),
        None => Err(AppError::NotFound(format!("Producer #{}", id))),
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/producers", web::get().to(index))
        .route("/producers", web::post().to(create))
        .route("/producers/{id}", web::put().to(update));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::admin_token;
    use actix_web::test;

    #[tokio::test(flavor = "current_thread")]
    async fn producers_index_authenticated() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/producers")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producer_create_success() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let name = format!("New Pressing Co {}", suffix);

        let req = test::TestRequest::post()
            .uri("/producers")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "producer_name": name,
                "description": "Premium vinyl"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(status, 201, "Expected 201, got {}: {:?}", status, body);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producer_create_empty_name() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/producers")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "producer_name": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producer_update_success() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let name = format!("Update Test {}", suffix);
        let new_name = format!("Updated {}", suffix);

        let id: i64 = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO producers (producer_name, description, created_at, updated_at)
               VALUES ($1, 'Original desc', NOW(), NOW())
               RETURNING id"
        )
        .bind(&name)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed producer");

        let req = test::TestRequest::put()
            .uri(&format!("/producers/{}", id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "producer_name": new_name,
                "description": "Updated desc"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["producer_name"], new_name);
        assert_eq!(body["description"], "Updated desc");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producer_update_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::put()
            .uri("/producers/99999999")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "producer_name": "Updated"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }
}
