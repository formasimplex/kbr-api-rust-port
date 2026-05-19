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
use sqlx::FromRow;
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::tenant_config::{
    CreateTenantConfigRequest, TenantConfig, TenantConfigResponse, UpdateTenantConfigRequest,
};

#[derive(Debug, FromRow)]
struct ConfigRow {
    tenant_id: Uuid,
    logo_url: Option<String>,
    short_name: String,
    long_name: String,
    footer_logo_url: Option<String>,
    contact_email: String,
    site_header_description: String,
    deleted_at: Option<chrono::NaiveDateTime>,
    #[sqlx(rename = "instaUrl")]
    insta_url: Option<String>,
    #[sqlx(rename = "twitterUrl")]
    twitter_url: Option<String>,
    #[sqlx(rename = "tiktokUrl")]
    tiktok_url: Option<String>,
    #[sqlx(rename = "spotifyId")]
    spotify_id: Option<String>,
    featured_artist_id: Option<i64>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<ConfigRow> for TenantConfig {
    fn from(row: ConfigRow) -> Self {
        TenantConfig {
            tenant_id: row.tenant_id,
            logo_url: row.logo_url,
            short_name: row.short_name,
            long_name: row.long_name,
            footer_logo_url: row.footer_logo_url,
            contact_email: row.contact_email,
            site_header_description: row.site_header_description,
            deleted_at: row.deleted_at.map(|d| d.and_utc()),
            insta_url: row.insta_url,
            twitter_url: row.twitter_url,
            tiktok_url: row.tiktok_url,
            spotify_id: row.spotify_id,
            featured_artist_id: row.featured_artist_id,
            mantine_theme: None,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

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
    let rows = sqlx::query_as::<_, ConfigRow>(
        r#"SELECT tenant_id, logo_url, short_name, long_name, footer_logo_url, contact_email,
           site_header_description, deleted_at, "instaUrl", "twitterUrl", "tiktokUrl", "spotifyId",
           featured_artist_id, created_at, updated_at
           FROM tenant_configs WHERE deleted_at IS NULL"#
    )
    .fetch_all(&state.db)
    .await?;

    let configs: Vec<TenantConfig> = rows.into_iter().map(|r| r.into()).collect();
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

    match sqlx::query_as::<_, ConfigRow>(
        r#"SELECT tenant_id, logo_url, short_name, long_name, footer_logo_url, contact_email,
           site_header_description, deleted_at, "instaUrl", "twitterUrl", "tiktokUrl", "spotifyId",
           featured_artist_id, created_at, updated_at
           FROM tenant_configs WHERE tenant_id = $1 AND deleted_at IS NULL"#
    )
    .bind(tenant_id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let config: TenantConfig = row.into();
            Ok(HttpResponse::Ok().json(config.to_response()))
        }
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

    let row = sqlx::query_as::<_, ConfigRow>(
        r#"INSERT INTO tenant_configs (tenant_id, logo_url, short_name, long_name, footer_logo_url,
           contact_email, site_header_description, deleted_at, "instaUrl", "twitterUrl", "tiktokUrl",
           "spotifyId", featured_artist_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, $8, $9, $10, $11, $12, $13, $14)
           RETURNING tenant_id, logo_url, short_name, long_name, footer_logo_url, contact_email,
           site_header_description, deleted_at, "instaUrl", "twitterUrl", "tiktokUrl", "spotifyId",
           featured_artist_id, created_at, updated_at"#
    )
    .bind(tenant_id)
    .bind(&body.logo_url)
    .bind(&body.short_name)
    .bind(&body.long_name)
    .bind(&body.footer_logo_url)
    .bind(&body.contact_email)
    .bind(&body.site_header_description)
    .bind(&body.insta_url)
    .bind(&body.twitter_url)
    .bind(&body.tiktok_url)
    .bind(&body.spotify_id)
    .bind(body.featured_artist_id)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let config: TenantConfig = row.into();
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

    let row = sqlx::query_as::<_, ConfigRow>(
        r#"UPDATE tenant_configs
           SET logo_url = COALESCE($1, logo_url),
               short_name = COALESCE($2, short_name),
               long_name = COALESCE($3, long_name),
               footer_logo_url = COALESCE($4, footer_logo_url),
               contact_email = COALESCE($5, contact_email),
               site_header_description = COALESCE($6, site_header_description),
               "instaUrl" = COALESCE($7, "instaUrl"),
               "twitterUrl" = COALESCE($8, "twitterUrl"),
               "tiktokUrl" = COALESCE($9, "tiktokUrl"),
               "spotifyId" = COALESCE($10, "spotifyId"),
               featured_artist_id = COALESCE($11, featured_artist_id),
               updated_at = $12
           WHERE tenant_id = $13 AND deleted_at IS NULL
           RETURNING tenant_id, logo_url, short_name, long_name, footer_logo_url, contact_email,
           site_header_description, deleted_at, "instaUrl", "twitterUrl", "tiktokUrl", "spotifyId",
           featured_artist_id, created_at, updated_at"#
    )
    .bind(&body.logo_url)
    .bind(&body.short_name)
    .bind(&body.long_name)
    .bind(&body.footer_logo_url)
    .bind(&body.contact_email)
    .bind(&body.site_header_description)
    .bind(&body.insta_url)
    .bind(&body.twitter_url)
    .bind(&body.tiktok_url)
    .bind(&body.spotify_id)
    .bind(body.featured_artist_id)
    .bind(now)
    .bind(tenant_id)
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(r) => {
            let config: TenantConfig = r.into();
            Ok(HttpResponse::Ok().json(config.to_response()))
        }
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

    let result = sqlx::query(
        r"UPDATE tenant_configs SET deleted_at = $1 WHERE tenant_id = $2 AND deleted_at IS NULL"
    )
    .bind(now)
    .bind(tenant_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() > 0 {
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
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";
    const TEST_DB_URL: &str = "postgresql://ws@localhost:5432/kbr_test";

    async fn get_state() -> AppState {
        let pool = sqlx::PgPool::connect(TEST_DB_URL)
            .await
            .expect("Failed to connect to test database");
        crate::test_utils::build_test_state(pool).await
    }

    fn token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn configs_index_authenticated() {
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
            .uri("/configs")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_create_success() {
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

        if let Some(tenant_id) = body["tenant_id"].as_str() {
            let _ = sqlx::query(&format!(
                "DELETE FROM tenant_configs WHERE tenant_id = '{}'::uuid",
                tenant_id
            ))
            .execute(&state.db)
            .await;
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_create_missing_fields() {
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
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_show_not_found() {
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
            .uri("/config/00000000-0000-0000-0000-000000000000")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }
}
