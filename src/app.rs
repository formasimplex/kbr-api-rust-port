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

/// Shared application state passed to all handlers via `web::Data<AppState>`.
///
/// # Field usage by handler module
///
/// | Field           | Handlers                                                                 |
/// |-----------------|--------------------------------------------------------------------------|
/// | `db`            | All handlers                                                             |
/// | `s3`            | artists, campaigns, dashboard, events, storage                           |
/// | `shopify`       | campaigns                                                                |
/// | `mailchimp`     | mailing                                                                  |
/// | `safe_browsing` | events, news                                                             |
/// | `email`         | jobs/email (background worker, not a handler)                            |
/// | `job_handle`    | mailing                                                                  |
/// | `jwt_secret`    | auth                                                                     |
/// | `cookie_builder`| auth                                                                     |
///
/// # Adding a new field
///
/// When adding a new field to AppState, you must update both construction
/// sites in `setup.rs` (the outer `Arc::new` and the inner `HttpServer::new`
/// closure). Consider whether the new dependency could be scoped to fewer
/// handlers using a dedicated extractor or a smaller state struct.
#[derive(Clone)]
pub struct AppState {
    /// PostgreSQL connection pool — used by every handler for data access.
    pub db: PgPool,

    /// S3 storage bucket — used for image/file URL generation.
    pub s3: Box<S3Bucket>,

    /// Shopify API client — used for campaign activation (optional).
    pub shopify: Option<ShopifyClient>,

    /// Mailchimp API client — used for subscriber sync (optional).
    pub mailchimp: Option<MailchimpClient>,

    /// Google Safe Browsing client — used for URL safety checks (optional).
    pub safe_browsing: Option<SafeBrowsingClient>,

    /// Email/SMTP client — used by background email jobs (optional).
    pub email: Option<EmailClient>,

    /// Background job dispatcher — used to enqueue async work.
    pub job_handle: JobHandle,

    /// JWT signing secret — used for token creation/verification in auth handlers.
    pub jwt_secret: String,

    /// Cookie builder for JWT cookies — used in auth handlers for session cookies.
    pub cookie_builder: Arc<ActixJwtCookie<Claims>>,
}
