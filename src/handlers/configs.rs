use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::tenant_config::{
    CreateTenantConfigRequest, TenantConfig, TenantConfigResponse, UpdateTenantConfigRequest,
};
use uuid::Uuid;

pub async fn index(_user: CurrentUser) -> Result<HttpResponse, AppError> {
    let configs = mock_all_configs();
    let responses: Vec<TenantConfigResponse> = configs.iter().map(|c| c.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(
    _user: CurrentUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let tenant_id = path.into_inner();
    match mock_find_config(tenant_id) {
        Some(c) => Ok(HttpResponse::Ok().json(c.to_response())),
        None => Err(AppError::NotFound(format!("Config #{}", tenant_id))),
    }
}

pub async fn create(
    _user: CurrentUser,
    body: web::Json<CreateTenantConfigRequest>,
) -> Result<HttpResponse, AppError> {
    if body.short_name.is_empty() {
        return Err(AppError::Validation("short_name is required".to_string()));
    }
    if body.long_name.is_empty() {
        return Err(AppError::Validation("long_name is required".to_string()));
    }
    if body.contact_email.is_empty() {
        return Err(AppError::Validation("contact_email is required".to_string()));
    }
    if body.site_header_description.is_empty() {
        return Err(AppError::Validation(
            "site_header_description is required".to_string(),
        ));
    }
    let config = TenantConfig {
        tenant_id: Uuid::new_v4(),
        logo_url: body.logo_url.clone(),
        short_name: body.short_name.clone(),
        long_name: body.long_name.clone(),
        footer_logo_url: body.footer_logo_url.clone(),
        contact_email: body.contact_email.clone(),
        site_header_description: body.site_header_description.clone(),
        deleted_at: None,
        insta_url: body.insta_url.clone(),
        twitter_url: body.twitter_url.clone(),
        tiktok_url: body.tiktok_url.clone(),
        spotify_id: body.spotify_id.clone(),
        featured_artist_id: body.featured_artist_id,
        mantine_theme: body.mantine_theme.clone(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(config.to_response()))
}

pub async fn update(
    _user: CurrentUser,
    path: web::Path<Uuid>,
    body: web::Json<UpdateTenantConfigRequest>,
) -> Result<HttpResponse, AppError> {
    let tenant_id = path.into_inner();
    match mock_find_config(tenant_id) {
        Some(mut c) => {
            if let Some(ref name) = body.short_name {
                c.short_name = name.clone();
            }
            if let Some(ref name) = body.long_name {
                c.long_name = name.clone();
            }
            if let Some(ref email) = body.contact_email {
                c.contact_email = email.clone();
            }
            if let Some(ref desc) = body.site_header_description {
                c.site_header_description = desc.clone();
            }
            if let Some(ref url) = body.logo_url {
                c.logo_url = Some(url.clone());
            }
            Ok(HttpResponse::Ok().json(c.to_response()))
        }
        None => Err(AppError::NotFound(format!("Config #{}", tenant_id))),
    }
}

pub async fn destroy(
    _user: CurrentUser,
    path: web::Path<Uuid>,
) -> Result<HttpResponse, AppError> {
    let tenant_id = path.into_inner();
    match mock_find_config(tenant_id) {
        Some(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Config #{} soft-deleted", tenant_id)
        }))),
        None => Err(AppError::NotFound(format!("Config #{}", tenant_id))),
    }
}

static TEST_UUID: Uuid = Uuid::nil();

fn mock_all_configs() -> Vec<TenantConfig> {
    vec![TenantConfig {
        tenant_id: TEST_UUID,
        logo_url: Some("https://example.com/logo.png".to_string()),
        short_name: "KBR".to_string(),
        long_name: "Keep Buying Records".to_string(),
        footer_logo_url: None,
        contact_email: "info@example.com".to_string(),
        site_header_description: "Welcome".to_string(),
        deleted_at: None,
        insta_url: None,
        twitter_url: None,
        tiktok_url: None,
        spotify_id: None,
        featured_artist_id: None,
        mantine_theme: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_find_config(id: Uuid) -> Option<TenantConfig> {
    if id == TEST_UUID {
        mock_all_configs().into_iter().next()
    } else {
        None
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/configs", web::get().to(index))
            .route("/config/{tenant_id}", web::get().to(show))
            .route("/configs", web::post().to(create))
            .route("/configs/{tenant_id}", web::put().to(update))
            .route("/configs/{tenant_id}", web::delete().to(destroy)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";

    fn token() -> String {
        crate::auth::jwt::encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn configs_index_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/configs")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_create_success() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/configs")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .set_json(serde_json::json!({
                "short_name": "Test",
                "long_name": "Test Config",
                "contact_email": "test@example.com",
                "site_header_description": "Welcome"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn config_create_missing_fields() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/configs")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .set_json(serde_json::json!({
                "short_name": "",
                "long_name": "",
                "contact_email": "",
                "site_header_description": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }
}
