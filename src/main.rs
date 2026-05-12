use actix_web::{HttpServer, web};
use dotenvy::dotenv;

mod db;
mod models;
mod auth;
mod handlers;
mod services;
mod responses;
mod error;

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
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
        .parse()
        .expect("Invalid BIND_ADDR");

    tracing::info!("Starting server on {}", addr);

    HttpServer::new(move || {
        actix_web::App::new()
            .app_data(web::Data::new(AppState { db: pool.clone() }))
            .configure(handlers::health::config_routes)
    })
    .bind(addr)?
    .run()
    .await
}
