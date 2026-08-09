//! Tenant config handlers
//!
//! Provides CRUD endpoints for tenant configuration management. All
//! endpoints require authentication.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/configs` | auth | List all non-deleted tenant configs |
//! | `show` | GET | `/v1/config/{tenant_id}` | auth | Retrieve a single tenant config by tenant_id |
//! | `create` | POST | `/v1/configs` | auth | Create a new tenant config |
//! | `update` | PUT | `/v1/configs/{tenant_id}` | auth | Update an existing tenant config |
//! | `destroy` | DELETE | `/v1/configs/{tenant_id}` | auth | Soft-delete a tenant config |

use actix_web::{web, HttpResponse};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::data::configs as data;
use crate::error::AppError;
use crate::models::tenant_config::{
    CreateTenantConfigRequest, TenantConfigResponse, UpdateTenantConfigRequest,
};

/// List all non-deleted tenant configs.
///
/// Requires authentication.
///
/// # Response
///
/// `200 OK` — JSON array of `TenantConfigResponse`
pub async fn index(
    state: web::Data<AppState>,
    _user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    let configs = data::list(&state.db).await?;
    let responses: Vec<TenantConfigResponse> = configs.iter().map(|c| c.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Retrieve a single tenant config by tenant_id.
///
/// Requires authentication. Only returns non-deleted configs.
///
/// # Response
///
/// `200 OK` — `TenantConfigResponse`
/// `404 Not Found` — config does not exist or is deleted
pub async fn show(
    state: web::Data<AppState>,
    _user: CurrentUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let tenant_id = path.into_inner();

    match data::by_tenant_id(&state.db, tenant_id).await? {
        Some(config) => Ok(HttpResponse::Ok().json(config.to_response())),
        None => Err(AppError::NotFound(format!("Config #{}", tenant_id))),
    }
}

/// Create a new tenant config.
///
/// Validates that short_name, long_name, contact_email, and
/// site_header_description are non-empty. Generates a new UUID for
/// the tenant_id. Requires authentication.
///
/// # Response
///
/// `201 Created` — `TenantConfigResponse` for the new config
/// `422 Unprocessable` — missing required fields
pub async fn create(
    state: web::Data<AppState>,
    _user: CurrentUser,
    body: web::Json<CreateTenantConfigRequest>,
) -> Result<HttpResponse, AppError> {
    if body.short_name.is_empty() {
        return Err(AppError::Validation("short_name is required".to_string()));
    }
    if body.long_name.is_empty() {
        return Err(AppError::Validation("long_name is required".to_string()));
    }
    if body.contact_email.is_empty() {
        return Err(AppError::Validation("contact_email is required".to_string()));
    }
    if body.site_header_description.is_empty() {
        return Err(AppError::Validation(
            "site_header_description is required".to_string(),
        ));
    }

    let tenant_id = Uuid::new_v4();
    let now = chrono::Utc::now().naive_utc();

    let config = data::create(&state.db, tenant_id, &body.logo_url, &body.short_name, &body.long_name, &body.footer_logo_url, &body.contact_email, &body.site_header_description, &body.insta_url, &body.twitter_url, &body.tiktok_url, &body.spotify_id, body.featured_artist_id, now).await?;
    Ok(HttpResponse::Created().json(config.to_response()))
}

/// Update an existing tenant config.
///
/// Uses COALESCE so only provided fields are updated. Requires
/// authentication. Only updates non-deleted configs.
///
/// # Response
///
/// `200 OK` — `TenantConfigResponse` with updated values
/// `404 Not Found` — config does not exist or is deleted
pub async fn update(
    state: web::Data<AppState>,
    _user: CurrentUser,
    path: web::Path<Uuid>,
    body: web::Json<UpdateTenantConfigRequest>,
) -> Result<HttpResponse, AppError> {
    let tenant_id = path.into_inner();
    let now = chrono::Utc::now().naive_utc();

    match data::update(&state.db, tenant_id, &body.logo_url, &body.short_name, &body.long_name, &body.footer_logo_url, &body.contact_email, &body.site_header_description, &body.insta_url, &body.twitter_url, &body.tiktok_url, &body.spotify_id, body.featured_artist_id, now).await? {
        Some(config) => Ok(HttpResponse::Ok().json(config.to_response())),
        None => Err(AppError::NotFound(format!("Config #{}", tenant_id))),
    }
}

/// Soft-delete a tenant config.
///
/// Sets `deleted_at` to the current timestamp. Requires authentication.
///
/// # Response
///
/// `200 OK` — confirmation message
/// `404 Not Found` — config does not exist or is already deleted
pub async fn destroy(
    state: web::Data<AppState>,
    _user: CurrentUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let tenant_id = path.into_inner();
    let now = chrono::Utc::now().naive_utc();

    if data::destroy(&state.db, tenant_id, now).await? {
        Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Config #{} soft-deleted", tenant_id)
        })))
    } else {
        Err(AppError::NotFound(format!("Config #{}", tenant_id)))
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/configs", web::get().to(index))
        .route("/config/{tenant_id}", web::get().to(show))
        .route("/configs", web::post().to(create))
        .route("/configs/{tenant_id}", web::put().to(update))
        .route("/configs/{tenant_id}", web::delete().to(destroy));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token_with_role;
    use crate::test_utils::TEST_SECRET;
    use actix_web::test;

    fn token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn configs_index_authenticated() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/configs")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_create_success() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/configs")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .set_json(serde_json::json!({
                "short_name": "TestKBR",
                "long_name": "Test Config",
                "contact_email": "test@example.com",
                "site_header_description": "Welcome"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["short_name"], "TestKBR");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_create_missing_fields() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/configs")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .set_json(serde_json::json!({
                "short_name": "",
                "long_name": "",
                "contact_email": "",
                "site_header_description": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_show_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/config/00000000-0000-0000-0000-000000000000")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }
}
