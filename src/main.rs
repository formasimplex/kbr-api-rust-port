#![allow(dead_code)]

use actix_web::{HttpServer, web};
use dotenvy::dotenv;

mod auth;
mod db;
mod error;
mod handlers;
mod models;
mod responses;
mod services;

use crate::db::pool::connect;

#[derive(Clone)]
struct AppState {
    db: sqlx::PgPool,
}

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
            .configure(handlers::health::config_routes)
            .configure(handlers::auth::config_routes)
            .configure(handlers::users::config_routes)
            .configure(handlers::permissions::config_routes)
            .configure(handlers::sign_up_trigger::config_routes)
            .configure(handlers::reset_trigger::config_routes)
            .configure(handlers::campaigns::config_routes)
            .configure(handlers::campaign_pages::config_routes)
            .configure(handlers::albums::config_routes)
            .configure(handlers::songs::config_routes)
            .configure(handlers::artists::config_routes)
            .configure(handlers::producers::config_routes)
            .configure(handlers::merchandise::config_routes)
            .configure(handlers::configs::config_routes)
            .configure(handlers::comments::config_routes)
            .configure(handlers::news::config_routes)
            .configure(handlers::playlists::config_routes)
            .configure(handlers::events::config_routes)
            .configure(handlers::event_attendees::config_routes)
            .configure(handlers::mailing::config_routes)
    })
    .bind(addr)?
    .run()
    .await
}
