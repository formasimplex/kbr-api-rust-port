//! Campaign activation service
//!
//! Encapsulates the full Shopify activation flow: validates campaign state,
//! creates a Shopify product with variants, publishes it, and updates the
//! campaign and campaign page with the resulting inventory references.

use sqlx::PgPool;

use crate::data::{campaign_pages, campaigns as campaign_data};
use crate::error::AppError;
use crate::models::campaign::Campaign;
use crate::services::shopify_graph_ql::ShopifyGraphQl;
use crate::services::shopify_client::ShopifyClient;
use crate::services::storage_service::{get_image_urls, S3Ops};

/// Context passed to the activation service.
///
/// Bundles the dependencies needed for the activation flow so the handler
/// stays thin and the service is independently testable.
pub struct ActivationContext<'a> {
    pub db: &'a PgPool,
    pub s3: &'a dyn S3Ops,
    pub shopify: &'a Option<ShopifyClient>,
}

/// Result of a successful campaign activation.
pub struct ActivationResult {
    pub campaign: Campaign,
    pub product_id: String,
    pub inventory_url: Option<String>,
}

pub struct CampaignActivationService;

impl CampaignActivationService {
    /// Execute the full campaign activation flow.
    ///
    /// # Steps
    ///
    /// 1. Parse and validate `start_date` string
    /// 2. Verify campaign exists
    /// 3. Find associated campaign page
    /// 4. Retrieve campaign page image from S3
    /// 5. Create Shopify product via GraphQL
    /// 6. Create product variant with inventory
    /// 7. Publish product to online store
    /// 8. Fetch final product details
    /// 9. Update campaign page with Shopify inventory references
    /// 10. Activate the campaign with computed date range
    pub async fn activate(
        ctx: &ActivationContext<'_>,
        campaign_id: i64,
        start_date_str: &str,
    ) -> Result<ActivationResult, AppError> {
        let (start_date, end_date) = Self::parse_dates(start_date_str)?;

        let sg = Self::build_shopify_client(ctx)?;

        let _campaign = Self::find_campaign(ctx.db, campaign_id).await?;
        let campaign_page = Self::find_campaign_page(ctx.db, campaign_id).await?;
        let image_url = Self::get_campaign_image(ctx, &campaign_page).await?;

        let (product_id, option_id) = Self::create_shopify_product(&sg, &campaign_page, &image_url).await?;
        let location_id = sg.get_location_id().await?;
        sg.create_product_variant(&product_id, &option_id, &location_id).await?;

        let publication_id = sg.get_publications_id().await?;
        sg.publish_product(&product_id, &publication_id).await?;

        let final_product = sg.get_product(&product_id).await?;
        let inventory_url = Self::extract_inventory_url(&final_product);
        let inventory_item_id = Some(product_id.clone());

        let now = chrono::Utc::now().naive_utc();
        campaign_pages::update_inventory(ctx.db, campaign_page.id, &inventory_item_id, &inventory_url, now).await?;
        let campaign = campaign_data::activate(ctx.db, campaign_id, start_date, end_date, now).await?;

        Ok(ActivationResult {
            campaign,
            product_id,
            inventory_url,
        })
    }

    /// Parse start_date string and compute end_date.
    fn parse_dates(start_date_str: &str) -> Result<(chrono::NaiveDateTime, chrono::NaiveDateTime), AppError> {
        let start_date = chrono::NaiveDate::parse_from_str(start_date_str, "%Y-%m-%d")
            .map_err(|_| AppError::Validation("start_date must be in YYYY-MM-DD format".to_string()))?
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| AppError::Validation("Invalid start_date".to_string()))?;

        let end_date = start_date + chrono::TimeDelta::days(crate::constants::CAMPAIGN_DURATION_DAYS);
        Ok((start_date, end_date))
    }

    /// Find the campaign by ID.
    async fn find_campaign(
        db: &PgPool,
        campaign_id: i64,
    ) -> Result<campaign_data::CampaignRow, AppError> {
        campaign_data::find_active(db, campaign_id).await?
            .ok_or_else(|| AppError::NotFound(format!("Campaign #{}", campaign_id)))
    }

    /// Find the campaign page for a given campaign.
    async fn find_campaign_page(
        db: &PgPool,
        campaign_id: i64,
    ) -> Result<crate::models::campaign_page::CampaignPage, AppError> {
        campaign_pages::by_campaign_id(db, campaign_id).await?
            .ok_or_else(|| AppError::NotFound(format!("CampaignPage for campaign #{}", campaign_id)))
    }

    /// Get the first image URL for the campaign page.
    async fn get_campaign_image(
        ctx: &ActivationContext<'_>,
        page: &crate::models::campaign_page::CampaignPage,
    ) -> Result<String, AppError> {
        let (image_urls, _) = get_image_urls(ctx.s3, ctx.db, "CampaignPage", page.id).await
            .map_err(|e| AppError::Shopify(format!("Failed to get campaign page image: {}", e)))?;

        Ok(image_urls.first()
            .ok_or_else(|| AppError::Shopify("Campaign page has no images attached".to_string()))?
            .clone())
    }

    /// Build ShopifyGraphQl client from context.
    fn build_shopify_client(ctx: &ActivationContext<'_>) -> Result<ShopifyGraphQl, AppError> {
        let shopify = ctx.shopify.as_ref().ok_or_else(|| {
            AppError::Shopify("Shopify client not configured".to_string())
        })?;
        Ok(ShopifyGraphQl::new(shopify.clone()))
    }

    /// Create a Shopify product and extract product/option IDs.
    async fn create_shopify_product(
        sg: &ShopifyGraphQl,
        page: &crate::models::campaign_page::CampaignPage,
        image_url: &str,
    ) -> Result<(String, String), AppError> {
        let page_title = page.title.as_deref().unwrap_or("Campaign");
        let page_description = page.description.as_deref().unwrap_or("");

        let product_resp = sg.create_campaign_product(page_title, page_description, image_url).await?;

        let product_id = product_resp
            .pointer("/data/productCreate/product/id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Shopify("Failed to get product ID from creation response".to_string()))?
            .to_string();

        let option_id = product_resp
            .pointer("/data/productCreate/product/options/0/id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Shopify("Failed to get product option ID".to_string()))?
            .to_string();

        Ok((product_id, option_id))
    }

    /// Extract the online store preview URL from the final product response.
    fn extract_inventory_url(product: &serde_json::Value) -> Option<String> {
        product
            .pointer("/data/product/onlineStorePreviewUrl")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_dates_valid() {
        let (start, end) = CampaignActivationService::parse_dates("2025-01-15").unwrap();
        assert_eq!(start.format("%Y-%m-%d %H:%M:%S").to_string(), "2025-01-15 00:00:00");
        assert_eq!(end - start, chrono::TimeDelta::days(crate::constants::CAMPAIGN_DURATION_DAYS));
    }

    #[test]
    fn parse_dates_invalid_format() {
        let err = CampaignActivationService::parse_dates("not-a-date").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn parse_dates_partial_date() {
        let err = CampaignActivationService::parse_dates("2025-13-01").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn extract_inventory_url_present() {
        let product = serde_json::json!({
            "data": {
                "product": {
                    "onlineStorePreviewUrl": "https://shop.example.com/products/test"
                }
            }
        });
        let url = CampaignActivationService::extract_inventory_url(&product);
        assert_eq!(url, Some("https://shop.example.com/products/test".to_string()));
    }

    #[test]
    fn extract_inventory_url_missing() {
        let product = serde_json::json!({
            "data": {
                "product": {}
            }
        });
        let url = CampaignActivationService::extract_inventory_url(&product);
        assert!(url.is_none());
    }
}
