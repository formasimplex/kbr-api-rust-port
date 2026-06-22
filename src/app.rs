use std::sync::Arc;

use actix_jc::ActixJwtCookie;
use s3::bucket::Bucket as S3Bucket;
use sqlx::PgPool;

use crate::auth::jwt::Claims;
use crate::jobs::JobHandle;
use crate::services::email_service::EmailClient;
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
    pub email: Option<EmailClient>,
    pub job_handle: JobHandle,
    pub jwt_secret: String,
    pub cookie_builder: Arc<ActixJwtCookie<Claims>>,
}
