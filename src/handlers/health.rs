use actix_web::{HttpResponse, web};

pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "ok",
        "service": "kbr-api-rust"
    }))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/health", web::get().to(health_check));
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    #[tokio::test(flavor = "current_thread")]
    async fn health_check_returns_ok() {
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get().uri("/health").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["status"], "ok");
        assert_eq!(body["service"], "kbr-api-rust");
    }
}
