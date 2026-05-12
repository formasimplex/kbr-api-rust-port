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
    cfg.service(
        web::scope("/v1")
            .route("/campaign_pages", web::get().to(index))
            .route("/campaign_pages/{id}", web::get().to(show)),
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
        AppState {
            db: sqlx::PgPool::connect(TEST_DB_URL)
                .await
                .expect("Failed to connect to test database"),
        }
    }

    fn admin_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    fn user_token() -> String {
        encode_token_with_role(99, TEST_SECRET, 3, Some("user".to_string())).unwrap()
    }

    async fn seed_user() -> i64 {
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
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed user")
    }

    async fn seed_artist(user_id: i64) -> i64 {
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
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed artist")
    }

    async fn seed_campaign(artist_id: i64) -> i64 {
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
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed campaign")
    }

    async fn seed_campaign_page(campaign_id: i64) -> i64 {
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
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed campaign page")
    }

    async fn cleanup_campaign_page(campaign_id: i64) {
        let _ = sqlx::query(r"DELETE FROM campaign_pages WHERE campaign_id = $1")
            .bind(campaign_id)
            .execute(&get_state().await.db)
            .await;
    }

    async fn cleanup_campaign(campaign_id: i64) {
        cleanup_campaign_page(campaign_id).await;
        let _ = sqlx::query(r"DELETE FROM campaigns WHERE id = $1")
            .bind(campaign_id)
            .execute(&get_state().await.db)
            .await;
    }

    async fn cleanup_artist(artist_id: i64) {
        let campaigns: Vec<i64> = sqlx::query_scalar(
            r"SELECT id FROM campaigns WHERE artist_id = $1",
        )
        .bind(artist_id)
        .fetch_all(&get_state().await.db)
        .await
        .unwrap_or_default();
        for cid in campaigns {
            cleanup_campaign(cid).await;
        }
        let _ = sqlx::query(r"DELETE FROM artists WHERE id = $1")
            .bind(artist_id)
            .execute(&get_state().await.db)
            .await;
    }

    async fn cleanup_user(user_id: i64) {
        let artists: Vec<i64> = sqlx::query_scalar(
            r"SELECT id FROM artists WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&get_state().await.db)
        .await
        .unwrap_or_default();
        for aid in artists {
            cleanup_artist(aid).await;
        }
        let _ = sqlx::query(r"DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&get_state().await.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaign_pages_index_admin() {
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
            .uri("/v1/campaign_pages")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaign_pages_index_forbidden() {
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
            .uri("/v1/campaign_pages")
            .insert_header(("Authorization", format!("Bearer {}", user_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaign_pages_show_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);

        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;
        let campaign_id = seed_campaign(artist_id).await;
        let page_id = seed_campaign_page(campaign_id).await;

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/campaign_pages/{}", page_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], page_id);

        cleanup_campaign_page(campaign_id).await;
        cleanup_campaign(campaign_id).await;
        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaign_pages_show_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM campaign_pages",
        )
        .fetch_one(&state.db)
        .await
        .expect("Failed to get max id");

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/campaign_pages/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }
}
