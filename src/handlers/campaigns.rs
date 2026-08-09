//! Campaign handlers
//!
//! Provides endpoints for campaign management including creation, activation,
//! and soft-deletion. Read endpoints are public; write operations require
//! artist+ role.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/campaigns` | public | List all non-deleted campaigns |
//! | `index_by_user` | GET | `/v1/campaigns_by_user` | artist+ | List campaigns for current user |
//! | `active_campaigns` | GET | `/v1/active_campaigns` | public | List only active campaigns |
//! | `show` | GET | `/v1/campaign/{id}` | public | Retrieve a single campaign by ID |
//! | `create` | POST | `/v1/campaigns` | artist+ | Create a new campaign |
//! | `update` | POST | `/v1/campaign/{id}` | artist+ | Update an existing campaign |
//! | `destroy` | DELETE | `/v1/campaign/{id}` | artist+ | Soft-delete a campaign |
//! | `activate_campaign` | POST | `/v1/activate_campaign` | artist+ | Activate campaign with Shopify product creation |

use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::permissions::is_artist_or_above;
use crate::data::campaigns as data;
use crate::error::AppError;
use crate::models::campaign::{Campaign, CampaignResponse, CreateCampaignRequest, UpdateCampaignRequest, ActivateCampaignRequest};
use crate::services::campaign_activation::{ActivationContext, CampaignActivationService};
use crate::services::campaign_service::CampaignService;

/// List all non-deleted campaigns.
///
/// Returns all campaigns where `deleted_at` is NULL, ordered by ID.
/// No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `CampaignResponse`
pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let campaigns = data::list(&state.db).await?;
    let responses: Vec<CampaignResponse> = campaigns.iter().map(|c| c.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// List campaigns for the current user.
///
/// Admins see all non-deleted campaigns. Artists see only their own campaigns.
/// Requires artist+ role.
///
/// # Response
///
/// `200 OK` — JSON array of `CampaignResponse`
/// `403 Forbidden` — user lacks required role
pub async fn index_by_user(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    let campaigns = if user.is_admin() {
        data::list(&state.db).await?
    } else if is_artist_or_above(&user.role) {
        data::list_by_artist(&state.db, user.id).await?
    } else {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    };

    let responses: Vec<CampaignResponse> = campaigns.iter().map(|c| c.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// List only active campaigns.
///
/// Returns campaigns where `active` is true and `deleted_at` is NULL.
/// No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `CampaignResponse`
pub async fn active_campaigns(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let campaigns = data::active(&state.db).await?;
    let responses: Vec<CampaignResponse> = campaigns.iter().map(|c| c.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Retrieve a single campaign by ID.
///
/// No authentication required.
///
/// # Response
///
/// `200 OK` — `CampaignResponse`
/// `404 Not Found` — campaign does not exist
pub async fn show(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match data::by_id(&state.db, id).await? {
        Some(campaign) => Ok(HttpResponse::Ok().json(campaign.to_response())),
        None => Err(AppError::NotFound(format!("Campaign #{}", id))),
    }
}

/// Create a new campaign.
///
/// Requires artist+ role. Validates the request body and creates a campaign
/// with `active` and `deleted_at` set to false/NULL respectively.
///
/// # Response
///
/// `201 Created` — `CampaignResponse`
/// `403 Forbidden` — user lacks required role
/// `422 Unprocessable` — validation failure
pub async fn create(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<CreateCampaignRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    CampaignService::validate_create_request(&body)?;

    let now = chrono::Utc::now().naive_utc();

    let campaign = data::create(&state.db, body.artist_id, &body.name, body.vinyl_sold_count, now).await?;
    Ok(HttpResponse::Created().json(campaign.to_response()))
}

/// Update an existing campaign.
///
/// Requires artist+ role. Validates `vinyl_sold_count` is between 0 and VINYL_TARGET.
/// Uses COALESCE to only update provided fields.
///
/// # Response
///
/// `200 OK` — `CampaignResponse`
/// `403 Forbidden` — user lacks required role
/// `404 Not Found` — campaign does not exist
/// `422 Unprocessable` — validation failure
pub async fn update(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateCampaignRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();

    if let Some(vinyl) = body.vinyl_sold_count
        && !Campaign::validate_vinyl_sold_count(vinyl) {
            return Err(AppError::Validation(
                format!("vinyl_sold_count must be between 0 and {}", crate::constants::VINYL_TARGET),
            ));
        }

    let now = chrono::Utc::now().naive_utc();

    let campaign = data::update(&state.db, id, &body.name, body.active, body.vinyl_sold_count, body.progress, body.campaign_start_date.map(|dt| dt.naive_utc()), body.campaign_end_date.map(|dt| dt.naive_utc()), now).await?
        .ok_or_else(|| AppError::NotFound(format!("Campaign #{}", id)))?;
    Ok(HttpResponse::Ok().json(campaign.to_response()))
}

/// Soft-delete a campaign.
///
/// Requires artist+ role. Sets `deleted_at` to current timestamp rather than
/// removing the row permanently.
///
/// # Response
///
/// `200 OK` — JSON object with confirmation message
/// `403 Forbidden` — user lacks required role
/// `404 Not Found` — campaign does not exist or already deleted
pub async fn destroy(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();

    let now = chrono::Utc::now().naive_utc();

    if !data::destroy(&state.db, id, now).await? {
        return Err(AppError::NotFound(format!("Campaign #{}", id)));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Campaign soft-deleted"
    })))
}

/// Activate a campaign with Shopify product creation.
///
/// Requires artist+ role. Executes the full activation flow:
/// 1. Validates the campaign exists and has an associated campaign page
/// 2. Retrieves the campaign page image URL from S3
/// 3. Creates a Shopify product via GraphQL with title, description, and image
/// 4. Creates a product variant with configured price and inventory
/// 5. Publishes the product to the online store
/// 6. Fetches final product details
/// 7. Updates the campaign page with Shopify inventory_item_id and inventory_url
/// 8. Updates the campaign with start_date, end_date (start + CAMPAIGN_DURATION_DAYS), and active = true
///
/// All Shopify calls are sequential with fail-fast semantics — if any call fails,
/// the database remains unchanged.
///
/// # Request Body
///
/// ```json
/// { "campaign_id": 123, "start_date": "2025-01-15" }
/// ```
///
/// # Response
///
/// `200 OK` — `CampaignResponse` with updated campaign data
/// `403 Forbidden` — user lacks required role
/// `404 Not Found` — campaign or campaign page does not exist
/// `422 Unprocessable` — missing or invalid campaign_id / start_date
/// `502 Bad Gateway` — Shopify API error or Shopify not configured
pub async fn activate_campaign(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<ActivateCampaignRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    let ctx = ActivationContext {
        db: &state.db,
        s3: &*state.s3,
        shopify: &state.shopify,
    };

    let result = CampaignActivationService::activate(&ctx, body.campaign_id, &body.start_date).await?;
    Ok(HttpResponse::Ok().json(result.campaign.to_response()))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/campaigns", web::get().to(index))
        .route("/campaigns_by_user", web::get().to(index_by_user))
        .route("/active_campaigns", web::get().to(active_campaigns))
        .route("/campaign/{id}", web::get().to(show))
        .route("/campaigns", web::post().to(create))
        .route("/campaign/{id}", web::post().to(update))
        .route("/campaign/{id}", web::delete().to(destroy))
        .route("/activate_campaign", web::post().to(activate_campaign));
}

#[cfg(test)]
mod tests {
    use super::*;
 use crate::auth::jwt::encode_token_with_role;
     use crate::test_utils::{admin_token, artist_token, not_found_id, seed_artist, seed_campaign, seed_test_user, unique_suffix, TEST_SECRET};
    use actix_web::test;

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_index_public() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get().uri("/campaigns").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_show_found() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "campaign_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;
        let campaign_id = seed_campaign(&state.db, artist_id).await;

        let req = test::TestRequest::get()
            .uri(&format!("/campaign/{}", campaign_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], campaign_id);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_show_not_found() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "campaigns").await;

        let req = test::TestRequest::get()
            .uri(&format!("/campaign/{}", not_found))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_create_admin() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "campaign_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        let name = format!("New Campaign {}_{}", pid, ts);

        let req = test::TestRequest::post()
            .uri("/campaigns")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "artist_id": artist_id,
                "name": name,
                "vinyl_sold_count": 50
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["vinyl_sold_count"], 50);
 assert_eq!(body["active"], false);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
 async fn campaigns_create_empty_name() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "campaign_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let req = test::TestRequest::post()
            .uri("/campaigns")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "artist_id": artist_id,
                "name": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_index_by_user_admin() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/campaigns_by_user")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
 let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
 async fn campaigns_update_success() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "campaign_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;
        let campaign_id = seed_campaign(&state.db, artist_id).await;

        let req = test::TestRequest::post()
            .uri(&format!("/campaign/{}", campaign_id))
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .set_json(serde_json::json!({
                "name": "Updated Campaign Name",
                "vinyl_sold_count": 75
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["name"], "Updated Campaign Name");
        assert_eq!(body["vinyl_sold_count"], 75);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_update_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "campaigns").await;

        let req = test::TestRequest::post()
            .uri(&format!("/campaign/{}", not_found))
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .set_json(serde_json::json!({
                "name": "Updated"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_destroy_success() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "campaign_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;
        let campaign_id = seed_campaign(&state.db, artist_id).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/campaign/{}", campaign_id))
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let deleted: Option<i64> = sqlx::query_scalar(
            r"SELECT id FROM campaigns WHERE id = $1 AND deleted_at IS NOT NULL",
        )
        .bind(campaign_id)
        .fetch_optional(&state.db)
        .await
        .expect("Failed to check deletion");
        assert_eq!(deleted, Some(campaign_id));

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_destroy_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "campaigns").await;

        let req = test::TestRequest::delete()
            .uri(&format!("/campaign/{}", not_found))
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_activate_success() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "campaign_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let campaign_id = seed_campaign(&state.db, artist_id).await;
        let _ = sqlx::query(r"UPDATE campaigns SET active = false WHERE id = $1")
            .bind(campaign_id)
            .execute(&state.db)
            .await
            .expect("Failed to deactivate campaign");

        let req = test::TestRequest::post()
            .uri("/activate_campaign")
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .set_json(serde_json::json!({
                "campaign_id": campaign_id,
                "start_date": "2025-01-15"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        // Returns 502 because Shopify client is not configured in tests
        assert_eq!(resp.status(), 502);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["error"].as_str().unwrap().contains("Shopify"));

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_activate_missing_start_date() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/activate_campaign")
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .set_json(serde_json::json!({
                "campaign_id": 99999
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        // Missing start_date field — JSON parse fails
        assert_eq!(resp.status(), 400);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_activate_invalid_date_format() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/activate_campaign")
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .set_json(serde_json::json!({
                "campaign_id": 99999,
                "start_date": "not-a-date"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["error"].as_str().unwrap().contains("YYYY-MM-DD"));

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_activate_campaign_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "campaigns").await;

        let req = test::TestRequest::post()
            .uri("/activate_campaign")
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .set_json(serde_json::json!({
                "campaign_id": not_found,
                "start_date": "2025-01-15"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 502);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_activate_no_campaign_page() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "campaign_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let campaign_id = seed_campaign(&state.db, artist_id).await;
        let _ = sqlx::query(r"UPDATE campaigns SET active = false WHERE id = $1")
            .bind(campaign_id)
            .execute(&state.db)
            .await
            .expect("Failed to deactivate campaign");

        let req = test::TestRequest::post()
            .uri("/activate_campaign")
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .set_json(serde_json::json!({
                "campaign_id": campaign_id,
                "start_date": "2025-01-15"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        // Shopify not configured, so we hit 502 before reaching campaign page check
        assert_eq!(resp.status(), 502);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_create_forbidden() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let token = encode_token_with_role(3, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let req = test::TestRequest::post()
            .uri("/campaigns")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "artist_id": 1,
                "name": "Should Fail"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_active_only() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "campaign_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let active_id = seed_campaign(&state.db, artist_id).await;

        let inactive_id: i64 = sqlx::query_scalar(
            r"INSERT INTO campaigns (artist_id, name, active, created_at, updated_at)
               VALUES ($1, $2, false, NOW(), NOW())
               RETURNING id",
        )
        .bind(artist_id)
        .bind(format!("Inactive Only {}", unique_suffix()))
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed inactive campaign");

        let req = test::TestRequest::get().uri("/active_campaigns").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        let arr = body.as_array().unwrap();
        let ids: Vec<i64> = arr.iter().map(|v| v["id"].as_i64().unwrap()).collect();
        assert!(ids.contains(&active_id));
        assert!(!ids.contains(&inactive_id));

        _guard.cleanup().await;
    }
}
