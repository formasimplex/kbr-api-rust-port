//! Campaign pages handlers
//!
//! Provides read-only endpoints for campaign page management. Index requires
//! admin role; show requires any authenticated user.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/campaign_pages` | admin | List all campaign pages |
//! | `show` | GET | `/v1/campaign_pages/{id}` | auth | Retrieve a single campaign page by ID |

use actix_web::{web, HttpResponse};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::campaign_page::{CampaignPage, CampaignPageResponse};

#[derive(Debug, FromRow)]
struct CampaignPageRow {
    id: i64,
    campaign_id: i64,
    title: Option<String>,
    description: Option<String>,
    page_type: Option<i32>,
    inventory_item_id: Option<String>,
    inventory_url: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<CampaignPageRow> for CampaignPage {
    fn from(row: CampaignPageRow) -> Self {
        CampaignPage {
            id: row.id,
            campaign_id: row.campaign_id,
            title: row.title,
            description: row.description,
            page_type: row.page_type,
            inventory_item_id: row.inventory_item_id,
            inventory_url: row.inventory_url,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const CAMPAIGN_PAGE_SELECT: &str =
    r#"SELECT id, campaign_id, title, description, page_type,
       inventory_item_id, inventory_url, created_at, updated_at FROM campaign_pages"#;

/// List all campaign pages.
///
/// Returns all campaign pages ordered by ID. Requires admin role.
///
/// # Response
///
/// `200 OK` — JSON array of `CampaignPageResponse`
/// `403 Forbidden` — user is not an admin
pub async fn index(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let rows = sqlx::query_as::<_, CampaignPageRow>(
        &format!("{} ORDER BY id", CAMPAIGN_PAGE_SELECT),
    )
    .fetch_all(&state.db)
    .await?;

    let pages: Vec<CampaignPage> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<CampaignPageResponse> = pages.iter().map(|p| p.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Retrieve a single campaign page by ID.
///
/// Requires authenticated user.
///
/// # Response
///
/// `200 OK` — `CampaignPageResponse`
/// `404 Not Found` — campaign page does not exist
pub async fn show(
    state: web::Data<AppState>,
    _user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match sqlx::query_as::<_, CampaignPageRow>(
        &format!("{} WHERE id = $1", CAMPAIGN_PAGE_SELECT),
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let page: CampaignPage = row.into();
            Ok(HttpResponse::Ok().json(page.to_response()))
        }
        None => Err(AppError::NotFound(format!("CampaignPage #{}", id))),
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/campaign_pages", web::get().to(index))
        .route("/campaign_pages/{id}", web::get().to(show));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{admin_token, not_found_id, user_token};
    use actix_web::test;

    async fn seed_user(pool: &sqlx::PgPool) -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO users (email, password_digest, role, created_at, updated_at)
               VALUES ($1, $2, $3, NOW(), NOW())
               RETURNING id",
        )
        .bind(format!("cpage_test_user_{}_{}@test.com", pid, ts))
        .bind("hashed_password_test".to_string())
        .bind(Some("artist".to_string()))
        .fetch_one(pool)
        .await
        .expect("Failed to seed user")
    }

    async fn seed_artist(pool: &sqlx::PgPool, user_id: i64) -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO artists (name, genre, bio, user_id, prospect, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               RETURNING id",
        )
        .bind(format!("CPage Test Artist {}_{}", pid, ts))
        .bind(Some("Electronic".to_string()))
        .bind(Some("A test artist for campaign pages".to_string()))
        .bind(Some(user_id))
        .bind(Some(false))
        .fetch_one(pool)
        .await
        .expect("Failed to seed artist")
    }

    async fn seed_campaign(pool: &sqlx::PgPool, artist_id: i64) -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO campaigns (artist_id, name, active, vinyl_sold_count, progress, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               RETURNING id",
        )
        .bind(artist_id)
        .bind(format!("CPage Campaign {}_{}", pid, ts))
        .bind(true)
        .bind(25_i32)
        .bind(50_i32)
        .fetch_one(pool)
        .await
        .expect("Failed to seed campaign")
    }

    async fn seed_campaign_page(pool: &sqlx::PgPool, campaign_id: i64) -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO campaign_pages (campaign_id, title, description, page_type, created_at, updated_at)
               VALUES ($1, $2, $3, $4, NOW(), NOW())
               RETURNING id",
        )
        .bind(campaign_id)
        .bind(format!("CPage Test {}_{}", pid, ts))
        .bind(Some("A test campaign page".to_string()))
        .bind(Some(0_i32))
        .fetch_one(pool)
        .await
        .expect("Failed to seed campaign page")
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaign_pages_index_admin() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/campaign_pages")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaign_pages_index_forbidden() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/campaign_pages")
            .insert_header(("Authorization", format!("Bearer {}", user_token(99))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaign_pages_show_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let user_id = seed_user(&state.db).await;
        let artist_id = seed_artist(&state.db, user_id).await;
        let campaign_id = seed_campaign(&state.db, artist_id).await;
        let page_id = seed_campaign_page(&state.db, campaign_id).await;

        let req = test::TestRequest::get()
            .uri(&format!("/campaign_pages/{}", page_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], page_id);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaign_pages_show_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "campaign_pages").await;

        let req = test::TestRequest::get()
            .uri(&format!("/campaign_pages/{}", not_found))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }
}
