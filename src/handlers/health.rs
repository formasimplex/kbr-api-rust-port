//! Health check handler
//!
//! Provides a lightweight endpoint for monitoring service availability.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `health_check` | GET | `/health` | public | Service health check |

use actix_web::{HttpResponse, web};

/// Service health check.
///
/// Returns a JSON payload with status and service name.
/// No authentication required.
///
/// # Response
///
/// `200 OK` — `{"status": "ok", "service": "kbr-api-rust"}`
pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "kbr-api-rust"
    }))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health_check));
}


