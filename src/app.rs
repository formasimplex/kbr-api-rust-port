use s3::bucket::Bucket as S3Bucket;
use sqlx::PgPool;

use crate::services::mailchimp_client::MailchimpClient;
use crate::services::safe_browsing::SafeBrowsingClient;
use crate::services::shopify_client::ShopifyClient;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub s3: Box<S3Bucket>,
    pub shopify: Option<ShopifyClient>,
    pub mailchimp: Option<MailchimpClient>,
    pub safe_browsing: Option<SafeBrowsingClient>,
}
