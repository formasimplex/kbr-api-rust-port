use dotenvy::dotenv;
use tokio::signal;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("kbr_api_rust=info".parse().unwrap()),
        )
        .init();

    dotenv().ok();

    rs_vips::Vips::init("kbr-api-rust").map_err(|e| {
        tracing::error!("Failed to initialize libvips: {}", e);
        std::io::Error::other(
            format!("libvips init failed: {}", e),
        )
    })?;

    let server = kbr_api_rust::setup::setup_web_server().await?;

    let server_handle = server.handle();

    tokio::select! {
        _ = server => {
            tracing::info!("Server stopped");
        }
        _ = graceful_shutdown() => {
            tracing::info!("Shutdown signal received, stopping server...");
            server_handle.stop(true).await;
        }
    }

    Ok(())
}

async fn graceful_shutdown() {
    let sigterm = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("SIGTERM")
            .recv()
            .await;
        tracing::info!("Received SIGTERM");
    };

    let sigint = async {
        signal::unix::signal(signal::unix::SignalKind::interrupt())
            .expect("SIGINT")
            .recv()
            .await;
        tracing::info!("Received SIGINT");
    };

    tokio::select! {
        _ = sigterm => {},
        _ = sigint => {},
    }
}
