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

