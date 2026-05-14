use actix_cors::Cors;

pub fn get_cors() -> Cors {
    let origins = std::env::var("CORS_ORIGINS").unwrap_or_default();

    if origins.trim().is_empty() {
        // Dev mode: allow any origin without credentials
        Cors::permissive().max_age(3600)
    } else {
        // Prod mode: specific origins with credentials
        let mut cors = Cors::default()
            .supports_credentials()
            .allowed_methods(["GET", "POST", "PUT", "DELETE", "OPTIONS"])
            .allowed_headers(["Content-Type", "Authorization", "Accept"])
            .max_age(3600);

        for origin in origins.split(',') {
            let origin = origin.trim().to_string();
            if !origin.is_empty() {
                cors = cors.allowed_origin(origin.as_str());
            }
        }

        cors
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App, HttpResponse, web};

    #[tokio::test(flavor = "current_thread")]
    async fn cors_allows_any_origin_when_env_not_set() {
        unsafe { std::env::remove_var("CORS_ORIGINS"); }

        let app = test::init_service(
            App::new()
                .wrap(get_cors())
                .service(
                    web::resource("/test").route(web::get().to(|| async {
                        HttpResponse::Ok().json(serde_json::json!({"ok": true}))
                    })),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/test")
            .insert_header(("Origin", "http://example.com"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);
        assert!(resp.headers().contains_key("Access-Control-Allow-Origin"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cors_uses_configured_origins() {
        unsafe { std::env::set_var("CORS_ORIGINS", "http://localhost:3000,https://app.example.com"); }

        let app = test::init_service(
            App::new()
                .wrap(get_cors())
                .service(
                    web::resource("/test").route(web::get().to(|| async {
                        HttpResponse::Ok().json(serde_json::json!({"ok": true}))
                    })),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/test")
            .insert_header(("Origin", "http://localhost:3000"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);
        assert_eq!(
            resp.headers().get("Access-Control-Allow-Origin").unwrap(),
            "http://localhost:3000"
        );

        unsafe { std::env::remove_var("CORS_ORIGINS"); }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cors_preflight_returns_ok() {
        unsafe { std::env::set_var("CORS_ORIGINS", "http://example.com"); }

        let app = test::init_service(
            App::new()
                .wrap(get_cors())
                .service(
                    web::resource("/test")
                        .route(web::get().to(|| async {
                            HttpResponse::Ok().json(serde_json::json!({"ok": true}))
                        }))
                        .route(
                            web::to(|| async { HttpResponse::NoContent() })
                                .method(actix_http::Method::OPTIONS),
                        ),
                ),
        )
        .await;

        let req = test::TestRequest::with_uri("/test")
            .method(actix_http::Method::OPTIONS)
            .insert_header(("Origin", "http://example.com"))
            .insert_header(("Access-Control-Request-Method", "POST"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert!(resp.status().is_success());
        assert!(resp.headers().contains_key("Access-Control-Allow-Origin"));
        assert!(resp.headers().contains_key("Access-Control-Allow-Methods"));

        unsafe { std::env::remove_var("CORS_ORIGINS"); }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cors_includes_credentials_header() {
        unsafe { std::env::set_var("CORS_ORIGINS", "http://example.com"); }

        let app = test::init_service(
            App::new()
                .wrap(get_cors())
                .service(
                    web::resource("/test").route(web::get().to(|| async {
                        HttpResponse::Ok().json(serde_json::json!({"ok": true}))
                    })),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/test")
            .insert_header(("Origin", "http://example.com"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);
        assert_eq!(
            resp.headers().get("Access-Control-Allow-Credentials").unwrap(),
            "true"
        );

        unsafe { std::env::remove_var("CORS_ORIGINS"); }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cors_trims_whitespace_from_origins() {
        unsafe { std::env::set_var("CORS_ORIGINS", " http://localhost:3000 , https://app.example.com "); }

        let app = test::init_service(
            App::new()
                .wrap(get_cors())
                .service(
                    web::resource("/test").route(web::get().to(|| async {
                        HttpResponse::Ok().json(serde_json::json!({"ok": true}))
                    })),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/test")
            .insert_header(("Origin", "http://localhost:3000"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), 200);
        assert_eq!(
            resp.headers().get("Access-Control-Allow-Origin").unwrap(),
            "http://localhost:3000"
        );

        unsafe { std::env::remove_var("CORS_ORIGINS"); }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cors_blocks_unconfigured_origin() {
        unsafe { std::env::set_var("CORS_ORIGINS", "http://localhost:3000"); }

        let app = test::init_service(
            App::new()
                .wrap(get_cors())
                .service(
                    web::resource("/test").route(web::get().to(|| async {
                        HttpResponse::Ok().json(serde_json::json!({"ok": true}))
                    })),
                ),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/test")
            .insert_header(("Origin", "http://evil.com"))
            .to_request();
        let resp = test::call_service(&app, req).await;

        // Non-allowed origin: no CORS headers added (browser will block)
        assert!(!resp.headers().contains_key("Access-Control-Allow-Origin"));

        unsafe { std::env::remove_var("CORS_ORIGINS"); }
    }
}
