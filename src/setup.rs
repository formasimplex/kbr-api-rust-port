use actix_web::{HttpServer, web, dev::Server};

use crate::app::AppState;
use crate::cors::get_cors;
use crate::db::pool::connect;
use crate::services::storage_service::{S3Config, create_s3_bucket};

pub async fn setup_web_server() -> std::io::Result<Server> {
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

    let server = HttpServer::new(move || {
        actix_web::App::new()
            .wrap(get_cors())
            .app_data(web::Data::new(AppState {
                db: pool.clone(),
                s3: s3.clone(),
            }))
            .configure(crate::handlers::data_api::config_routes)
            .configure(crate::handlers::health::config_routes)
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
            .configure(crate::handlers::dashboard::config_routes)
    })
    .bind(addr)?
    .run();

    Ok(server)
}
