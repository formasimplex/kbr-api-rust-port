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
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::permission::{
    CreatePermissionRequest, Permission, PermissionResponse, UpdatePermissionRequest,
};
use crate::services::permission_service::PermissionService;

#[derive(Debug, FromRow)]
struct PermissionRow {
    id: i64,
    resource: Option<String>,
    can_create: bool,
    can_read: bool,
    can_update: bool,
    can_delete: bool,
    user_id: i64,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<PermissionRow> for Permission {
    fn from(row: PermissionRow) -> Self {
        Permission {
            id: row.id,
            resource: row.resource,
            can_create: row.can_create,
            can_read: row.can_read,
            can_update: row.can_update,
            can_delete: row.can_delete,
            user_id: row.user_id,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

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
    let rows = sqlx::query_as::<_, PermissionRow>(
        r"SELECT id, resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at FROM permissions"
    )
    .fetch_all(&state.db)
    .await?;

    let perms: Vec<Permission> = rows.into_iter().map(|r| r.into()).collect();
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

    match sqlx::query_as::<_, PermissionRow>(
        r"SELECT id, resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at FROM permissions WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let perm: Permission = row.into();
            Ok(HttpResponse::Ok().json(perm.to_response()))
        }
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

    let row = sqlx::query_as::<_, PermissionRow>(
        r"INSERT INTO permissions (resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at"
    )
    .bind(&body.resource)
    .bind(body.can_create.unwrap_or(false))
    .bind(body.can_read.unwrap_or(true))
    .bind(body.can_update.unwrap_or(false))
    .bind(body.can_delete.unwrap_or(false))
    .bind(user.id)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let perm: Permission = row.into();
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

    let row = sqlx::query_as::<_, PermissionRow>(
        r"UPDATE permissions
           SET can_create = COALESCE($1, can_create),
               can_read = COALESCE($2, can_read),
               can_update = COALESCE($3, can_update),
               can_delete = COALESCE($4, can_delete),
               updated_at = $5
           WHERE id = $6
           RETURNING id, resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at"
    )
    .bind(body.can_create)
    .bind(body.can_read)
    .bind(body.can_update)
    .bind(body.can_delete)
    .bind(now)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(r) => {
            let perm: Permission = r.into();
            Ok(HttpResponse::Ok().json(perm.to_response()))
        }
        None => Err(AppError::NotFound(format!("Permission #{}", id))),
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
    use crate::auth::jwt::encode_token_with_role;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";
    const TEST_DB_URL: &str = "postgresql://ws@localhost:5432/kbr_test";

    async fn get_state() -> AppState {
        let pool = sqlx::PgPool::connect(TEST_DB_URL)
            .await
            .expect("Failed to connect to test database");

        let config = crate::services::storage_service::S3Config::from_env()
            .unwrap_or_else(|_| crate::services::storage_service::S3Config {
                access_key: "test".to_string(),
                secret_key: "test".to_string(),
                endpoint: "https://test.test".to_string(),
                bucket_name: "test".to_string(),
                region: "us-east-1".to_string(),
            });
        let s3 = crate::services::storage_service::create_s3_bucket(&config).unwrap_or_else(|_| {
            let creds = s3::creds::Credentials::new(Some("test"), Some("test"), None, None, None).unwrap();
            s3::bucket::Bucket::new("test", s3::region::Region::Custom { region: "us-east-1".to_string(), endpoint: "https://test.test".to_string() }, creds)
                .unwrap()
                .with_path_style()
        });

        AppState { db: pool, s3, shopify: None, mailchimp: None, safe_browsing: None }
    }

    fn admin_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    fn user_token() -> String {
        encode_token_with_role(2, TEST_SECRET, 3, None).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_index_admin() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/permissions")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_index_non_admin_forbidden() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/permissions")
            .insert_header(("Authorization", format!("Bearer {}", user_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_resources_admin() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

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
    async fn permissions_show_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let max_id: i64 = sqlx::query_scalar(r"SELECT COALESCE(MAX(id), 0) FROM permissions")
            .fetch_one(&state.db)
            .await
            .expect("Failed to get max id");

        let req = test::TestRequest::get()
            .uri(&format!("/v1/permissions/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_create_admin() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);

        let _ = sqlx::query(
            r"INSERT INTO users (id, email, password_digest, created_at, updated_at)
               VALUES (1, 'permadmin@example.com', 'hashed', NOW(), NOW())
               ON CONFLICT (id) DO NOTHING"
        )
        .execute(&state.db)
        .await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

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
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["resource"], "Album");

        if let Some(id) = body["id"].as_i64() {
            let _ = sqlx::query(&format!("DELETE FROM permissions WHERE id = {}", id))
                .execute(&state.db)
                .await;
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn permissions_create_invalid_resource() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

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
    async fn permissions_update_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let max_id: i64 = sqlx::query_scalar(r"SELECT COALESCE(MAX(id), 0) FROM permissions")
            .fetch_one(&state.db)
            .await
            .expect("Failed to get max id");

        let req = test::TestRequest::put()
            .uri(&format!("/v1/permissions/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "can_delete": true
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }
}
