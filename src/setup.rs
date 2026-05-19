use std::sync::Arc;

use actix_governor::{Governor, GovernorConfigBuilder};
use actix_web::{HttpServer, web, dev::Server};

use crate::app::AppState;
use crate::auth::jwt::Claims;
use crate::cors::get_cors;
use crate::db::pool::connect;
use crate::jobs::{JobHandle, spawn_worker, spawn_retry_worker};
use crate::services::email_service::EmailClient;
use crate::services::mailchimp_client::MailchimpClient;
use crate::services::safe_browsing::SafeBrowsingClient;
use crate::services::shopify_client::ShopifyClient;
use crate::services::storage_service::{S3Config, create_s3_bucket};

pub async fn setup_web_server() -> std::io::Result<Server> {
    let pool = connect().await.map_err(|e| {
        tracing::error!("Failed to connect to database: {}", e);
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    })?;

    crate::db::migrate::run_migrations(&pool).await.map_err(|e| {
        tracing::error!("Failed to run database migrations: {}", e);
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    })?;

    let s3_config = S3Config::from_env().map_err(|e| {
        tracing::error!("Failed to load S3 config: {}", e);
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    })?;

    let s3 = create_s3_bucket(&s3_config).map_err(|e| {
        tracing::error!("Failed to create S3 bucket: {}", e);
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    })?;

    let shopify = ShopifyClient::new_from_env();
    if shopify.is_none() {
        tracing::warn!(
            "Shopify client not configured (SHOPIFY_STORE/SHOPIFY_ACCESS_TOKEN not set). \
             Campaign activation will fail at runtime."
        );
    }

    let mailchimp = MailchimpClient::new_from_env();
    if mailchimp.is_none() {
        tracing::warn!(
            "Mailchimp client not configured (MAILCHIMP_API_KEY not set). \
             Subscriber sync will be skipped."
        );
    }

    let safe_browsing = SafeBrowsingClient::new_from_env();
    if safe_browsing.is_none() {
        tracing::warn!(
            "Safe Browsing client not configured (GOOGLE_SAFE_BROWSING_API_KEY not set). \
             URL safety checks will be skipped."
        );
    }

    let email = EmailClient::new_from_env();
    if email.is_none() {
        tracing::warn!(
            "Email client not configured (SMTP_HOST not set). \
             Emails will not be sent."
        );
    }

    let (job_handle, job_rx) = JobHandle::new(256);

    let jwt_secret = std::env::var("JWT_SECRET").map_err(|_| {
        tracing::error!("JWT_SECRET not configured");
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "JWT_SECRET not configured",
        )
    })?;

    let jwt_secret_static: &'static str = jwt_secret.leak();

    let cookie_builder = Arc::new(
        actix_jc::ActixJwtCookie::<Claims>::new()
            .cookie_name("jwt_cookie")
            .jwt_key(jwt_secret_static)
            .expiration(3 * 24 * 60 * 60)
    );

    let governor_conf = GovernorConfigBuilder::default()
        .requests_per_second(10)
        .burst_size(30)
        .finish()
        .ok_or_else(|| {
            tracing::error!("Failed to create governor config");
            std::io::Error::new(std::io::ErrorKind::Other, "Failed to create governor config")
        })?;

    let addr: std::net::SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8181".to_string())
        .parse()
        .expect("Invalid BIND_ADDR");

    tracing::info!("Starting server on {}", addr);

    let state = Arc::new(AppState {
        db: pool.clone(),
        s3: s3.clone(),
        shopify: shopify.clone(),
        mailchimp: mailchimp.clone(),
        safe_browsing: safe_browsing.clone(),
        email: email.clone(),
        job_handle: job_handle.clone(),
        jwt_secret: jwt_secret_static.to_string(),
        cookie_builder: cookie_builder.clone(),
    });

    spawn_worker(job_rx, state.clone());
    spawn_retry_worker(pool.clone());

    let governor_conf = governor_conf.clone();
    let cookie_builder = cookie_builder.clone();
    let jwt_secret_static = jwt_secret_static;

    let server = HttpServer::new(move || {
        actix_web::App::new()
            .wrap(get_cors())
            .wrap(Governor::new(&governor_conf))
            .app_data(web::Data::new(cookie_builder.clone()))
            .app_data(web::Data::new(AppState {
                db: pool.clone(),
                s3: s3.clone(),
                shopify: shopify.clone(),
                mailchimp: mailchimp.clone(),
                safe_browsing: safe_browsing.clone(),
                email: email.clone(),
                job_handle: job_handle.clone(),
                jwt_secret: jwt_secret_static.to_string(),
                cookie_builder: cookie_builder.clone(),
            }))
            .configure(crate::handlers::health::config_routes)
            .service(
                web::scope("/v1")
                    .configure(crate::handlers::data_api::config_routes)
                    .configure(crate::handlers::auth::config_routes)
                    .configure(crate::handlers::users::config_routes)
                    .configure(crate::handlers::permissions::config_routes)
                    .configure(crate::handlers::sign_up_trigger::config_routes)
                    .configure(crate::handlers::reset_trigger::config_routes)
                    .configure(crate::handlers::campaigns::config_routes)
                    .configure(crate::handlers::campaign_pages::config_routes)
                    .configure(crate::handlers::albums::config_routes)
                    .configure(crate::handlers::songs::config_routes)
                    .configure(crate::handlers::artists::config_routes)
                    .configure(crate::handlers::producers::config_routes)
                    .configure(crate::handlers::merchandise::config_routes)
                    .configure(crate::handlers::configs::config_routes)
                    .configure(crate::handlers::comments::config_routes)
                    .configure(crate::handlers::news::config_routes)
                    .configure(crate::handlers::playlists::config_routes)
                    .configure(crate::handlers::events::config_routes)
                    .configure(crate::handlers::event_attendees::config_routes)
                    .configure(crate::handlers::mailing::config_routes)
                    .configure(crate::handlers::webhook::config_routes)
                    .configure(crate::handlers::storage::config_routes)
                    .configure(crate::handlers::dashboard::config_routes),
            )
    })
    .bind(addr)?
    .run();

    Ok(server)
}
