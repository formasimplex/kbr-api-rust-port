//! Webhook handlers
//!
//! Provides endpoints for external webhook integrations, including
//! progress updates, customer data requests, and GDPR-related data
//! redaction. All endpoints are public.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `update_progress` | POST | `/v1/webhook/update_progress` | public | Update progress for a record |
//! | `customers_data_request` | POST | `/v1/webhook/customers_data_request` | public | Handle a customer data request |
//! | `customers_redact` | POST | `/v1/webhook/customers_redact` | public | Handle a customer data redaction request |
//! | `shop_redact` | POST | `/v1/webhook/shop_redact` | public | Handle a shop data redaction request |

use actix_web::{web, HttpResponse};
use serde::Deserialize;
use sqlx::FromRow;

use crate::app::AppState;
use crate::error::AppError;

/// Top-level webhook request body containing inventory data.
#[derive(Debug, Deserialize)]
pub struct WebhookInventoryParams {
    webhook: WebhookPayload,
}

/// Webhook payload with Shopify inventory item details.
#[derive(Debug, Deserialize)]
pub struct WebhookPayload {
    inventory_item_id: String,
    #[allow(dead_code)]
    location_id: Option<i64>,
    available: Option<String>,
    #[allow(dead_code)]
    updated_at: Option<String>,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct CampaignPageRow {
    id: i64,
    campaign_id: i64,
    inventory_item_id: Option<String>,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
struct CampaignRow {
    id: i64,
    artist_id: i64,
    name: Option<String>,
    active: Option<bool>,
    vinyl_sold_count: Option<i32>,
    campaign_start_date: Option<chrono::NaiveDateTime>,
    campaign_end_date: Option<chrono::NaiveDateTime>,
    progress: Option<i32>,
    album_id: Option<i64>,
    deleted_at: Option<chrono::NaiveDateTime>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

/// Update campaign progress from a Shopify inventory webhook.
///
/// Looks up the campaign page by `inventory_item_id`, calculates vinyl
/// sold as `target (100) - available`, and computes progress as the
/// average of vinyl progress and time-based progress.
///
/// # Response
///
/// `200 OK` — `{"status": "ok"}` (always succeeds, no-op if not found)
pub async fn update_progress(
    state: web::Data<AppState>,
    body: web::Json<WebhookInventoryParams>,
) -> Result<HttpResponse, AppError> {
    let inventory_item_id = &body.webhook.inventory_item_id;
    let available: i32 = body
        .webhook
        .available
        .as_deref()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0);

    let campaign_page = sqlx::query_as::<_, CampaignPageRow>(
        r#"SELECT id, campaign_id, inventory_item_id FROM campaign_pages WHERE inventory_item_id = $1"#
    )
    .bind(inventory_item_id)
    .fetch_optional(&state.db)
    .await?;

    let Some(page) = campaign_page else {
        return Ok(HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })));
    };

    let campaign = sqlx::query_as::<_, CampaignRow>(
        r#"SELECT id, artist_id, name, active, vinyl_sold_count, campaign_start_date,
           campaign_end_date, progress, album_id, deleted_at, created_at, updated_at
           FROM campaigns WHERE id = $1"#
    )
    .bind(page.campaign_id)
    .fetch_optional(&state.db)
    .await?;

    let Some(campaign) = campaign else {
        return Ok(HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })));
    };

    let target = 100i32;
    let vinyl_sold = target - available;

    let progress = if let (Some(start), Some(end)) =
        (campaign.campaign_start_date, campaign.campaign_end_date)
    {
        let today = chrono::Utc::now().naive_utc();
        let total_days = (end - start).num_days().max(1);
        let days_passed = (today - start).num_days().max(0);
        let vinyl_progress = (vinyl_sold as f64 / target as f64) * 100.0;
        let time_progress = (days_passed as f64 / total_days as f64) * 100.0;
        let avg = (vinyl_progress + time_progress) / 2.0;
        avg.clamp(0.0, 100.0) as i32
    } else {
        let vinyl_progress = (vinyl_sold as f64 / target as f64) * 100.0;
        vinyl_progress.clamp(0.0, 100.0) as i32
    };

    let now = chrono::Utc::now().naive_utc();

    sqlx::query(
        r#"UPDATE campaigns SET vinyl_sold_count = $1, progress = $2, updated_at = $3 WHERE id = $4"#
    )
    .bind(vinyl_sold)
    .bind(progress)
    .bind(now)
    .bind(campaign.id)
    .execute(&state.db)
    .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })))
}

/// Handle a customer data request webhook (GDPR compliance).
///
/// Currently a stub that acknowledges the request.
///
/// # Response
///
/// `200 OK` — `{"status": "ok"}`
pub async fn customers_data_request() -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })))
}

/// Handle a customer data redaction webhook (GDPR right to erasure).
///
/// Currently a stub that acknowledges the request.
///
/// # Response
///
/// `200 OK` — `{"status": "ok"}`
pub async fn customers_redact() -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })))
}

/// Handle a shop-level data redaction webhook.
///
/// Currently a stub that acknowledges the request.
///
/// # Response
///
/// `200 OK` — `{"status": "ok"}`
pub async fn shop_redact() -> Result<HttpResponse, AppError> {
    Ok(HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/webhook/update_progress", web::post().to(update_progress))
        .route("/webhook/customers_data_request", web::post().to(customers_data_request))
        .route("/webhook/customers_redact", web::post().to(customers_redact))
        .route("/webhook/shop_redact", web::post().to(shop_redact));
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    use std::sync::atomic::{AtomicI64, Ordering};
    static TEST_COUNTER: AtomicI64 = AtomicI64::new(0);

    fn unique_suffix() -> String {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        format!("{}_{}", ts, count)
    }

async fn get_state() -> AppState {
        let pool = sqlx::PgPool::connect(crate::test_utils::test_db_url())
            .await
            .expect("Failed to connect to test database");
        crate::test_utils::build_test_state(pool).await
    }

    async fn seed_campaign_with_page() -> (i64, i64, String) {
        let now = chrono::Utc::now().naive_utc();
        let start = now;
        let end = now + chrono::TimeDelta::days(30);
        let inventory_item_id = format!("shopify_item_{}", unique_suffix());
        let artist_name = format!("Webhook Artist {}", unique_suffix());

        let artist_id: i64 = sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO artists (name, created_at, updated_at) VALUES ($1, $2, $3) RETURNING id"#
        )
        .bind(&artist_name)
        .bind(&now)
        .bind(&now)
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed artist");

        let campaign_id: i64 = sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO campaigns (artist_id, name, active, vinyl_sold_count, campaign_start_date, campaign_end_date, progress, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING id"#
        )
        .bind(artist_id)
        .bind(format!("Test Campaign {}", unique_suffix()))
        .bind(true)
        .bind(0i32)
        .bind(&start)
        .bind(&end)
        .bind(0i32)
        .bind(&now)
        .bind(&now)
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed campaign");

        let page_id: i64 = sqlx::query_scalar::<_, i64>(
            r#"INSERT INTO campaign_pages (campaign_id, title, description, inventory_item_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING id"#
        )
        .bind(campaign_id)
        .bind(format!("Test Page {}", unique_suffix()))
        .bind("Test page description")
        .bind(&inventory_item_id)
        .bind(&now)
        .bind(&now)
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed campaign page");

        (campaign_id, page_id, inventory_item_id)
    }

    async fn cleanup_campaign(campaign_id: i64) {
        let _ = sqlx::query(r#"DELETE FROM campaign_pages WHERE campaign_id = $1"#)
            .bind(campaign_id)
            .execute(&get_state().await.db)
            .await;
        let _ = sqlx::query(r#"DELETE FROM campaigns WHERE id = $1"#)
            .bind(campaign_id)
            .execute(&get_state().await.db)
            .await;
        let _ = sqlx::query(r#"DELETE FROM artists WHERE name LIKE 'Webhook Artist %'"#)
            .execute(&get_state().await.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_update_progress_updates_campaign() {
        unsafe { std::env::set_var("DATABASE_URL", crate::test_utils::test_db_url()); }
        let state = web::Data::new(get_state().await);

        let (campaign_id, _page_id, inventory_item_id) = seed_campaign_with_page().await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/webhook/update_progress")
            .set_json(serde_json::json!({
                "webhook": {
                    "inventory_item_id": inventory_item_id,
                    "available": "70"
                }
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "ok");

        let vinyl_sold: i32 = sqlx::query_scalar(
            r#"SELECT vinyl_sold_count FROM campaigns WHERE id = $1"#
        )
        .bind(campaign_id)
        .fetch_one(&state.db)
        .await
        .expect("Failed to read campaign");
        assert_eq!(vinyl_sold, 30);

        cleanup_campaign(campaign_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_update_progress_no_campaign_page() {
        unsafe { std::env::set_var("DATABASE_URL", crate::test_utils::test_db_url()); }
        let state = web::Data::new(get_state().await);

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/webhook/update_progress")
            .set_json(serde_json::json!({
                "webhook": {
                    "inventory_item_id": "nonexistent-item-id",
                    "available": "50"
                }
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_update_progress_zero_available() {
        unsafe { std::env::set_var("DATABASE_URL", crate::test_utils::test_db_url()); }
        let state = web::Data::new(get_state().await);

        let (campaign_id, _page_id, inventory_item_id) = seed_campaign_with_page().await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/webhook/update_progress")
            .set_json(serde_json::json!({
                "webhook": {
                    "inventory_item_id": inventory_item_id,
                    "available": "0"
                }
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let vinyl_sold: i32 = sqlx::query_scalar(
            r#"SELECT vinyl_sold_count FROM campaigns WHERE id = $1"#
        )
        .bind(campaign_id)
        .fetch_one(&state.db)
        .await
        .expect("Failed to read campaign");
        assert_eq!(vinyl_sold, 100);

        cleanup_campaign(campaign_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_customers_data_request() {
        unsafe { std::env::set_var("DATABASE_URL", crate::test_utils::test_db_url()); }
        let state = web::Data::new(get_state().await);

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/webhook/customers_data_request")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_customers_redact() {
        unsafe { std::env::set_var("DATABASE_URL", crate::test_utils::test_db_url()); }
        let state = web::Data::new(get_state().await);

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/webhook/customers_redact")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "ok");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_shop_redact() {
        unsafe { std::env::set_var("DATABASE_URL", crate::test_utils::test_db_url()); }
        let state = web::Data::new(get_state().await);

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/webhook/shop_redact")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "ok");
    }
}
