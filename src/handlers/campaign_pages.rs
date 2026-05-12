use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::campaign_page::{CampaignPage, CampaignPageResponse};

pub async fn index(user: CurrentUser) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let pages = mock_all_pages();
    let responses: Vec<CampaignPageResponse> = pages.iter().map(|p| p.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(
    _user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_page(id) {
        Some(p) => Ok(HttpResponse::Ok().json(p.to_response())),
        None => Err(AppError::NotFound(format!("CampaignPage #{}", id))),
    }
}

fn mock_all_pages() -> Vec<CampaignPage> {
    vec![CampaignPage {
        id: 1,
        campaign_id: 1,
        title: Some("Campaign Page".to_string()),
        description: Some("Description".to_string()),
        page_type: Some(0),
        inventory_item_id: None,
        inventory_url: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_find_page(id: i64) -> Option<CampaignPage> {
    if id == 1 {
        Some(CampaignPage {
            id: 1,
            campaign_id: 1,
            title: Some("Campaign Page".to_string()),
            description: Some("Description".to_string()),
            page_type: Some(0),
            inventory_item_id: None,
            inventory_url: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    } else {
        None
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/campaign_pages", web::get().to(index))
            .route("/campaign_pages/{id}", web::get().to(show)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";

    fn admin_token() -> String {
        crate::auth::jwt::encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaign_pages_show_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/campaign_pages/1")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaign_pages_index_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/campaign_pages")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }
}
