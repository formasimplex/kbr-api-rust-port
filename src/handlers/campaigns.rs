use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_artist_or_above;
use crate::error::AppError;
use crate::models::campaign::{Campaign, CampaignResponse, CreateCampaignRequest, UpdateCampaignRequest};
use crate::services::campaign_service::CampaignService;

pub async fn index() -> Result<HttpResponse, AppError> {
    let campaigns = mock_active_campaigns();
    let responses: Vec<CampaignResponse> = campaigns.iter().map(|c| c.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn index_by_user(user: CurrentUser) -> Result<HttpResponse, AppError> {
    let campaigns = if user.is_admin() {
        mock_all_campaigns()
    } else if is_artist_or_above(&user.role) {
        mock_artist_campaigns(user.id)
    } else {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    };
    let responses: Vec<CampaignResponse> = campaigns.iter().map(|c| c.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn active_campaigns() -> Result<HttpResponse, AppError> {
    let campaigns = mock_active_campaigns();
    let responses: Vec<CampaignResponse> = campaigns.iter().map(|c| c.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(path: web::Path<i64>) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_campaign(id) {
        Some(c) => Ok(HttpResponse::Ok().json(c.to_response())),
        None => Err(AppError::NotFound(format!("Campaign #{}", id))),
    }
}

pub async fn create(
    user: CurrentUser,
    body: web::Json<CreateCampaignRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    CampaignService::validate_create_request(&body)?;

    let campaign = Campaign {
        id: 999,
        artist_id: body.artist_id,
        name: Some(body.name.clone()),
        active: Some(false),
        vinyl_sold_count: body.vinyl_sold_count.or(Some(0)),
        campaign_start_date: None,
        campaign_end_date: None,
        progress: Some(0),
        album_id: None,
        deleted_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(campaign.to_response()))
}

pub async fn update(
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateCampaignRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();
    match mock_find_campaign(id) {
        Some(mut c) => {
            if let Some(ref name) = body.name {
                c.name = Some(name.clone());
            }
            if let Some(active) = body.active {
                c.active = Some(active);
            }
            if let Some(vinyl) = body.vinyl_sold_count {
                if !Campaign::validate_vinyl_sold_count(vinyl) {
                    return Err(AppError::Validation(
                        "vinyl_sold_count must be between 0 and 100".to_string(),
                    ));
                }
                c.vinyl_sold_count = Some(vinyl);
            }
            Ok(HttpResponse::Ok().json(c.to_response()))
        }
        None => Err(AppError::NotFound(format!("Campaign #{}", id))),
    }
}

pub async fn destroy(
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();
    match mock_find_campaign(id) {
        Some(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": "Campaign soft-deleted"
        }))),
        None => Err(AppError::NotFound(format!("Campaign #{}", id))),
    }
}

pub async fn activate_campaign(
    user: CurrentUser,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let campaign_id = body.get("campaign_id").and_then(|v| v.as_i64()).ok_or_else(|| {
        AppError::Validation("campaign_id is required".to_string())
    })?;
    match mock_find_campaign(campaign_id) {
        Some(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": "Campaign activated",
            "campaign_id": campaign_id
        }))),
        None => Err(AppError::NotFound(format!("Campaign #{}", campaign_id))),
    }
}

fn mock_active_campaigns() -> Vec<Campaign> {
    vec![Campaign {
        id: 1,
        artist_id: 1,
        name: Some("Active Campaign".to_string()),
        active: Some(true),
        vinyl_sold_count: Some(25),
        campaign_start_date: Some(chrono::Utc::now()),
        campaign_end_date: None,
        progress: Some(50),
        album_id: Some(1),
        deleted_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_all_campaigns() -> Vec<Campaign> {
    mock_active_campaigns()
}

fn mock_artist_campaigns(_user_id: i64) -> Vec<Campaign> {
    mock_active_campaigns()
}

fn mock_find_campaign(id: i64) -> Option<Campaign> {
    if id == 1 {
        Some(Campaign {
            id: 1,
            artist_id: 1,
            name: Some("Test Campaign".to_string()),
            active: Some(true),
            vinyl_sold_count: Some(25),
            campaign_start_date: Some(chrono::Utc::now()),
            campaign_end_date: None,
            progress: Some(50),
            album_id: Some(1),
            deleted_at: None,
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
            .route("/campaigns", web::get().to(index))
            .route("/campaigns_by_user", web::get().to(index_by_user))
            .route("/active_campaigns", web::get().to(active_campaigns))
            .route("/campaign/{id}", web::get().to(show))
            .route("/campaigns", web::post().to(create))
            .route("/campaign/{id}", web::post().to(update))
            .route("/campaign/{id}", web::delete().to(destroy))
            .route("/activate_campaign", web::post().to(activate_campaign)),
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
    async fn campaigns_index_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/campaigns").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_show_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/campaign/1").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_show_not_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/campaign/9999").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_create_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/campaigns")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "artist_id": 1,
                "name": "New Campaign",
                "vinyl_sold_count": 50
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_create_empty_name() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/campaigns")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "artist_id": 1,
                "name": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_index_by_user_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/campaigns_by_user")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }
}
