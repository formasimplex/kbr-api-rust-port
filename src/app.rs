use s3::bucket::Bucket as S3Bucket;
use sqlx::PgPool;

use crate::services::shopify_client::ShopifyClient;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub s3: Box<S3Bucket>,
    pub shopify: Option<ShopifyClient>,
}
