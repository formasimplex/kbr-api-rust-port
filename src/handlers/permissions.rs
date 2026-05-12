use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::permission::{
    CreatePermissionRequest, Permission, PermissionResponse, UpdatePermissionRequest,
};
use crate::services::permission_service::PermissionService;

pub async fn index(user: CurrentUser) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let perms = mock_all_permissions();
    let responses: Vec<PermissionResponse> = perms.iter().map(|p| p.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn index_resources(user: CurrentUser) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let resources = PermissionService::get_all_resource_names();
    Ok(HttpResponse::Ok().json(resources))
}

pub async fn show(
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();
    match mock_find_permission(id) {
        Some(p) => Ok(HttpResponse::Ok().json(p.to_response())),
        None => Err(AppError::NotFound(format!("Permission #{}", id))),
    }
}

pub async fn create(
    user: CurrentUser,
    body: web::Json<CreatePermissionRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    PermissionService::validate_create_request(&body)?;

    let perm = Permission {
        id: 999,
        resource: Some(body.resource.clone()),
        can_create: body.can_create.unwrap_or(false),
        can_read: body.can_read.unwrap_or(true),
        can_update: body.can_update.unwrap_or(false),
        can_delete: body.can_delete.unwrap_or(false),
        user_id: user.id,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(perm.to_response()))
}

pub async fn update(
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdatePermissionRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();
    match mock_find_permission(id) {
        Some(mut p) => {
            if let Some(v) = body.can_create {
                p.can_create = v;
            }
            if let Some(v) = body.can_read {
                p.can_read = v;
            }
            if let Some(v) = body.can_update {
                p.can_update = v;
            }
            if let Some(v) = body.can_delete {
                p.can_delete = v;
            }
            Ok(HttpResponse::Ok().json(p.to_response()))
        }
        None => Err(AppError::NotFound(format!("Permission #{}", id))),
    }
}

fn mock_all_permissions() -> Vec<Permission> {
    vec![
        Permission {
            id: 1,
            resource: Some("Campaign".to_string()),
            can_create: true,
            can_read: true,
            can_update: true,
            can_delete: true,
            user_id: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
        Permission {
            id: 2,
            resource: Some("News".to_string()),
            can_create: true,
            can_read: true,
            can_update: true,
            can_delete: true,
            user_id: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        },
    ]
}

fn mock_find_permission(id: i64) -> Option<Permission> {
    match id {
        1 => Some(Permission {
            id: 1,
            resource: Some("Campaign".to_string()),
            can_create: true,
            can_read: true,
            can_update: true,
            can_delete: true,
            user_id: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }),
        2 => Some(Permission {
            id: 2,
            resource: Some("News".to_string()),
            can_create: true,
            can_read: true,
            can_update: true,
            can_delete: true,
            user_id: 1,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }),
        _ => None,
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/permissions", web::get().to(index))
            .route("/permissions_resources", web::get().to(index_resources))
            .route("/permissions", web::post().to(create))
            .route("/permissions/{id}", web::get().to(show))
            .route("/permissions/{id}", web::put().to(update)),
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
    async fn permissions_index_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/permissions")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_index_non_admin_forbidden() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/permissions")
            .insert_header(("Authorization", format!("Bearer {}", user_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_resources_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/permissions_resources")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body: Vec<String> = test::read_body_json(resp).await;
        assert!(body.contains(&"Campaign".to_string()));
        assert!(body.contains(&"News".to_string()));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_show_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/permissions/1")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_show_not_found() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/permissions/9999")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_create_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/permissions")
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
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_create_invalid_resource() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/permissions")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "resource": "InvalidResource"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_update_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::put()
            .uri("/v1/permissions/1")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "can_delete": true
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }
}
