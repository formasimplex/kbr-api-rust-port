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

use crate::app::AppState;
use crate::data::{campaign_pages, campaigns};
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

    let Some(page) = campaign_pages::find_by_inventory_item_id(&state.db, inventory_item_id).await? else {
        return Ok(HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })));
    };

    let Some(campaign) = campaigns::by_id(&state.db, page.campaign_id).await? else {
        return Ok(HttpResponse::Ok().json(serde_json::json!({ "status": "ok" })));
    };

    let target = crate::constants::VINYL_TARGET;
    let vinyl_sold = target - available;

    let progress = if let (Some(start), Some(end)) =
        (campaign.campaign_start_date, campaign.campaign_end_date)
    {
        let today = chrono::Utc::now().naive_utc();
        let start = start.naive_utc();
        let end = end.naive_utc();
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

    campaigns::update_progress(&state.db, campaign.id, vinyl_sold, progress, now).await?;

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
    use crate::test_utils::{seed_artist, seed_campaign, seed_campaign_page, unique_suffix};
    use actix_web::test;

    async fn seed_campaign_with_page(pool: &sqlx::PgPool) -> (i64, i64, String) {
        let now = chrono::Utc::now().naive_utc();
        let start = now;
        let end = now + chrono::TimeDelta::days(30);
        let inventory_item_id = format!("shopify_item_{}", unique_suffix());

        let artist_id = seed_artist(pool, None).await;
        let campaign_id = seed_campaign(pool, artist_id).await;

        // Update campaign with date range for webhook tests
        let _ = sqlx::query(
            r#"UPDATE campaigns SET campaign_start_date = $1, campaign_end_date = $2 WHERE id = $3"#
        )
        .bind(&start)
        .bind(&end)
        .bind(campaign_id)
        .execute(pool)
        .await;

        let page_id = seed_campaign_page(pool, campaign_id).await;

        // Update page with inventory_item_id for webhook tests
        let _ = sqlx::query(
            r#"UPDATE campaign_pages SET inventory_item_id = $1 WHERE id = $2"#
        )
        .bind(&inventory_item_id)
        .bind(page_id)
        .execute(pool)
        .await;

        (campaign_id, page_id, inventory_item_id)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_update_progress_updates_campaign() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (campaign_id, _page_id, inventory_item_id) = seed_campaign_with_page(&state.db).await;

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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_update_progress_no_campaign_page() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_update_progress_zero_available() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (campaign_id, _page_id, inventory_item_id) = seed_campaign_with_page(&state.db).await;

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
        assert_eq!(vinyl_sold, crate::constants::VINYL_TARGET);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_customers_data_request() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/webhook/customers_data_request")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "ok");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_customers_redact() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/webhook/customers_redact")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "ok");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn webhook_shop_redact() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/webhook/shop_redact")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "ok");

        _guard.cleanup().await;
    }
}
