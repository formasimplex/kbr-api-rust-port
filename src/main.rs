use actix_web::{HttpServer, web};
use dotenvy::dotenv;

use kbr_api_rust::app::AppState;
use kbr_api_rust::db::pool::connect;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("kbr_api_rust=info".parse().unwrap()),
        )
        .init();

    dotenv().ok();

    let pool = connect().await.map_err(|e| {
        tracing::error!("Failed to connect to database: {}", e);
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    })?;

    let addr: std::net::SocketAddr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8181".to_string())
        .parse()
        .expect("Invalid BIND_ADDR");

    tracing::info!("Starting server on {}", addr);

    HttpServer::new(move || {
        actix_web::App::new()
            .app_data(web::Data::new(AppState { db: pool.clone() }))
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
    })
    .bind(addr)?
    .run()
    .await
}
