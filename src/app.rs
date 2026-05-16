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

#[cfg(test)]
impl AppState {
    pub fn for_tests(db: PgPool, s3: Box<S3Bucket>) -> Self {
        let secret = "test-secret-for-unit-tests-only".to_string();
        let secret_static: &'static str = secret.leak();
        Self {
            db,
            s3,
            shopify: None,
            mailchimp: None,
            safe_browsing: None,
            email: None,
            job_handle: JobHandle::inline(),
            jwt_secret: secret_static.to_string(),
            cookie_builder: Arc::new(
                ActixJwtCookie::<Claims>::new()
                    .cookie_name("jwt_cookie")
                    .jwt_key(secret_static)
                    .permanent()
            ),
        }
    }
}
