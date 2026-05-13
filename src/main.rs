use actix_web::{HttpServer, web};
use dotenvy::dotenv;

use kbr_api_rust::app::AppState;
use kbr_api_rust::db::pool::connect;
use kbr_api_rust::services::storage_service::{create_s3_bucket, S3Config};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("kbr_api_rust=info".parse().unwrap()),
        )
        .init();

    dotenv().ok();

    rs_vips::Vips::init("kbr-api-rust")
        .map_err(|e| {
            tracing::error!("Failed to initialize libvips: {}", e);
            std::io::Error::new(std::io::ErrorKind::Other, format!("libvips init failed: {}", e))
        })?;

    let pool = connect().await.map_err(|e| {
        tracing::error!("Failed to connect to database: {}", e);
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

    let addr: std::net::SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8181".to_string())
        .parse()
        .expect("Invalid BIND_ADDR");

    tracing::info!("Starting server on {}", addr);

    HttpServer::new(move || {
        actix_web::App::new()
            .app_data(web::Data::new(AppState {
                db: pool.clone(),
                s3: s3.clone(),
            }))
            .configure(kbr_api_rust::handlers::data_api::config_routes)
            .configure(kbr_api_rust::handlers::health::config_routes)
            .configure(kbr_api_rust::handlers::auth::config_routes)
            .configure(kbr_api_rust::handlers::users::config_routes)
            .configure(kbr_api_rust::handlers::permissions::config_routes)
            .configure(kbr_api_rust::handlers::sign_up_trigger::config_routes)
            .configure(kbr_api_rust::handlers::reset_trigger::config_routes)
            .configure(kbr_api_rust::handlers::campaigns::config_routes)
            .configure(kbr_api_rust::handlers::campaign_pages::config_routes)
            .configure(kbr_api_rust::handlers::albums::config_routes)
            .configure(kbr_api_rust::handlers::songs::config_routes)
            .configure(kbr_api_rust::handlers::artists::config_routes)
            .configure(kbr_api_rust::handlers::producers::config_routes)
            .configure(kbr_api_rust::handlers::merchandise::config_routes)
            .configure(kbr_api_rust::handlers::configs::config_routes)
            .configure(kbr_api_rust::handlers::comments::config_routes)
            .configure(kbr_api_rust::handlers::news::config_routes)
            .configure(kbr_api_rust::handlers::playlists::config_routes)
            .configure(kbr_api_rust::handlers::events::config_routes)
            .configure(kbr_api_rust::handlers::event_attendees::config_routes)
            .configure(kbr_api_rust::handlers::mailing::config_routes)
            .configure(kbr_api_rust::handlers::webhook::config_routes)
            .configure(kbr_api_rust::handlers::storage::config_routes)
    })
    .bind(addr)?
    .run()
    .await
}
