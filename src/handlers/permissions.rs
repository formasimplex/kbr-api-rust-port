//! Permission management handlers
//!
//! Provides CRUD endpoints for role-based permissions. All endpoints require
//! admin authentication. Permissions control what resources users can access.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/permissions` | admin | List all permissions |
//! | `index_resources` | GET | `/v1/permissions_resources` | admin | List available permission resources |
//! | `show` | GET | `/v1/permissions/{id}` | admin | Retrieve a single permission |
//! | `create` | POST | `/v1/permissions` | admin | Create a new permission |
//! | `update` | PUT | `/v1/permissions/{id}` | admin | Update permission flags |

use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::data::permissions as data;
use crate::error::AppError;
use crate::models::permission::{
    CreatePermissionRequest, PermissionResponse, UpdatePermissionRequest,
};
use crate::services::permission_service::PermissionService;

/// List all permissions.
///
/// Returns all permission records. Requires admin role.
///
/// # Response
///
/// `200 OK` — JSON array of `PermissionResponse`
/// `403 Forbidden` — non-admin user
pub async fn index(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let perms = data::list(&state.db).await?;
    let responses: Vec<PermissionResponse> = perms.iter().map(|p| p.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// List available permission resource names.
///
/// Returns the set of valid resource identifiers that can be used when
/// creating permissions. Requires admin role.
///
/// # Response
///
/// `200 OK` — JSON array of resource name strings
/// `403 Forbidden` — non-admin user
pub async fn index_resources(user: CurrentUser) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let resources = PermissionService::get_all_resource_names();
    Ok(HttpResponse::Ok().json(resources))
}

/// Retrieve a single permission by ID.
///
/// Requires admin role.
///
/// # Response
///
/// `200 OK` — `PermissionResponse`
/// `403 Forbidden` — non-admin user
/// `404 Not Found` — permission does not exist
pub async fn show(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();

    match data::by_id(&state.db, id).await? {
        Some(perm) => Ok(HttpResponse::Ok().json(perm.to_response())),
        None => Err(AppError::NotFound(format!("Permission #{}", id))),
    }
}

/// Create a new permission record.
///
/// Validates the resource name against the known set of resources.
/// Boolean flags default to `false` for create/update/delete and `true` for read.
/// Requires admin role.
///
/// # Response
///
/// `201 Created` — `PermissionResponse` for the new permission
/// `403 Forbidden` — non-admin user
/// `422 Unprocessable Entity` — invalid resource name
pub async fn create(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<CreatePermissionRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    PermissionService::validate_create_request(&body)?;

    let now = chrono::Utc::now().naive_utc();

    let perm = data::create(&state.db, &body.resource, body.can_create.unwrap_or(false), body.can_read.unwrap_or(true), body.can_update.unwrap_or(false), body.can_delete.unwrap_or(false), user.id, now).await?;
    Ok(HttpResponse::Created().json(perm.to_response()))
}

/// Update permission flags for an existing permission.
///
/// Performs a partial update using COALESCE — only provided fields are changed.
/// Requires admin role.
///
/// # Response
///
/// `200 OK` — updated `PermissionResponse`
/// `403 Forbidden` — non-admin user
/// `404 Not Found` — permission does not exist
pub async fn update(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdatePermissionRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();
    let now = chrono::Utc::now().naive_utc();

    match data::update(&state.db, id, body.can_create, body.can_read, body.can_update, body.can_delete, now).await? {
        Some(perm) => Ok(HttpResponse::Ok().json(perm.to_response())),
        None => Err(AppError::NotFound(format!("Permission #{}", id))),
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/permissions", web::get().to(index))
        .route("/permissions_resources", web::get().to(index_resources))
        .route("/permissions", web::post().to(create))
        .route("/permissions/{id}", web::get().to(show))
        .route("/permissions/{id}", web::put().to(update));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{admin_token, not_found_id, user_token};
    use actix_web::test;

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_index_admin() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/permissions")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_index_non_admin_forbidden() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/permissions")
            .insert_header(("Authorization", format!("Bearer {}", user_token(2))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_resources_admin() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/permissions_resources")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: Vec<String> = test::read_body_json(resp).await;
        assert!(body.contains(&"Campaign".to_string()));
        assert!(body.contains(&"News".to_string()));

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_show_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "permissions").await;

        let req = test::TestRequest::get()
            .uri(&format!("/permissions/{}", not_found))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_create_admin() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let _ = sqlx::query(
            r"INSERT INTO users (id, email, password_digest, created_at, updated_at)
               VALUES (1, 'permadmin@example.com', 'hashed', NOW(), NOW())
               ON CONFLICT (id) DO NOTHING"
        )
        .execute(&state.db)
        .await;

        let req = test::TestRequest::post()
            .uri("/permissions")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "resource": "Album",
                "can_create": true,
                "can_read": true,
                "can_update": false,
                "can_delete": false
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["resource"], "Album");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_create_invalid_resource() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/permissions")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "resource": "InvalidResource"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_update_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "permissions").await;

        let req = test::TestRequest::put()
            .uri(&format!("/permissions/{}", not_found))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "can_delete": true
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }
}
