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
//! | `activate_campaign` | POST | `/v1/activate_campaign` | artist+ | Activate a campaign |

use actix_web::{web, HttpResponse};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_artist_or_above;
use crate::error::AppError;
use crate::models::campaign::{Campaign, CampaignResponse, CreateCampaignRequest, UpdateCampaignRequest};
use crate::services::campaign_service::CampaignService;

#[derive(Debug, FromRow)]
struct CampaignRow {
    id: i64,
    artist_id: i64,
    name: Option<String>,
    active: Option<bool>,
    vinyl_sold_count: Option<i32>,
    campaign_start_date: Option<chrono::NaiveDateTime>,
    campaign_end_date: Option<chrono::NaiveDateTime>,
    progress: Option<i32>,
    album_id: Option<i64>,
    deleted_at: Option<chrono::NaiveDateTime>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<CampaignRow> for Campaign {
    fn from(row: CampaignRow) -> Self {
        Campaign {
            id: row.id,
            artist_id: row.artist_id,
            name: row.name,
            active: row.active,
            vinyl_sold_count: row.vinyl_sold_count,
            campaign_start_date: row.campaign_start_date.map(|dt| dt.and_utc()),
            campaign_end_date: row.campaign_end_date.map(|dt| dt.and_utc()),
            progress: row.progress,
            album_id: row.album_id,
            deleted_at: row.deleted_at.map(|dt| dt.and_utc()),
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const CAMPAIGN_SELECT: &str =
    r#"SELECT id, artist_id, name, active, vinyl_sold_count,
       campaign_start_date, campaign_end_date, progress, album_id,
       deleted_at, created_at, updated_at FROM campaigns"#;

/// List all non-deleted campaigns.
///
/// Returns all campaigns where `deleted_at` is NULL, ordered by ID.
/// No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `CampaignResponse`
pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let rows = sqlx::query_as::<_, CampaignRow>(
        &format!("{} WHERE deleted_at IS NULL ORDER BY id", CAMPAIGN_SELECT),
    )
    .fetch_all(&state.db)
    .await?;

    let campaigns: Vec<Campaign> = rows.into_iter().map(|r| r.into()).collect();
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
        sqlx::query_as::<_, CampaignRow>(
            &format!("{} WHERE deleted_at IS NULL ORDER BY id", CAMPAIGN_SELECT),
        )
        .fetch_all(&state.db)
        .await?
    } else if is_artist_or_above(&user.role) {
        sqlx::query_as::<_, CampaignRow>(
            &format!("{} WHERE artist_id = $1 AND deleted_at IS NULL ORDER BY id", CAMPAIGN_SELECT),
        )
        .bind(user.id)
        .fetch_all(&state.db)
        .await?
    } else {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    };

    let campaigns: Vec<Campaign> = campaigns.into_iter().map(|r| r.into()).collect();
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
    let rows = sqlx::query_as::<_, CampaignRow>(
        &format!("{} WHERE active = true AND deleted_at IS NULL ORDER BY id", CAMPAIGN_SELECT),
    )
    .fetch_all(&state.db)
    .await?;

    let campaigns: Vec<Campaign> = rows.into_iter().map(|r| r.into()).collect();
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
    match sqlx::query_as::<_, CampaignRow>(
        &format!("{} WHERE id = $1", CAMPAIGN_SELECT),
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let campaign: Campaign = row.into();
            Ok(HttpResponse::Ok().json(campaign.to_response()))
        }
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

    let row = sqlx::query_as::<_, CampaignRow>(
        r#"INSERT INTO campaigns (artist_id, name, active, vinyl_sold_count, campaign_start_date, campaign_end_date, progress, album_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING id, artist_id, name, active, vinyl_sold_count,
               campaign_start_date, campaign_end_date, progress, album_id,
               deleted_at, created_at, updated_at"#,
    )
    .bind(body.artist_id)
    .bind(&body.name)
    .bind(false)
    .bind(body.vinyl_sold_count)
    .bind::<Option<chrono::NaiveDateTime>>(None)
    .bind::<Option<chrono::NaiveDateTime>>(None)
    .bind(0_i32)
    .bind::<Option<i64>>(None)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let campaign: Campaign = row.into();
    Ok(HttpResponse::Created().json(campaign.to_response()))
}

/// Update an existing campaign.
///
/// Requires artist+ role. Validates `vinyl_sold_count` is between 0 and 100.
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
                "vinyl_sold_count must be between 0 and 100".to_string(),
            ));
        }

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, CampaignRow>(
        r#"UPDATE campaigns SET
                name = COALESCE($1, name),
                active = COALESCE($2, active),
                vinyl_sold_count = COALESCE($3, vinyl_sold_count),
                progress = COALESCE($4, progress),
                campaign_start_date = COALESCE($5, campaign_start_date),
                campaign_end_date = COALESCE($6, campaign_end_date),
                updated_at = $7
            WHERE id = $8
            RETURNING id, artist_id, name, active, vinyl_sold_count,
            campaign_start_date, campaign_end_date, progress, album_id,
            deleted_at, created_at, updated_at"#,
    )
    .bind(body.name.as_deref())
    .bind(body.active)
    .bind(body.vinyl_sold_count)
    .bind(body.progress)
    .bind(body.campaign_start_date.map(|dt| dt.naive_utc()))
    .bind(body.campaign_end_date.map(|dt| dt.naive_utc()))
    .bind(now)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Campaign #{}", id)))?;

    let campaign: Campaign = row.into();
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

    let result = sqlx::query(
        r#"UPDATE campaigns SET deleted_at = $1, updated_at = $2
           WHERE id = $3 AND deleted_at IS NULL"#,
    )
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Campaign #{}", id)));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Campaign soft-deleted"
    })))
}

/// Activate a campaign.
///
/// Requires artist+ role. Sets `active` to true for the given campaign ID.
/// Request body must contain `campaign_id`.
///
/// # Response
///
/// `200 OK` — JSON object with confirmation message and campaign ID
/// `403 Forbidden` — user lacks required role
/// `404 Not Found` — campaign does not exist
/// `422 Unprocessable` — missing or invalid campaign_id
pub async fn activate_campaign(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let campaign_id = body.get("campaign_id").and_then(|v| v.as_i64()).ok_or_else(|| {
        AppError::Validation("campaign_id is required".to_string())
    })?;

    let now = chrono::Utc::now().naive_utc();

    let result = sqlx::query(
        r#"UPDATE campaigns SET active = true, updated_at = $1
           WHERE id = $2 AND deleted_at IS NULL"#,
    )
    .bind(now)
    .bind(campaign_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Campaign #{}", campaign_id)));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Campaign activated",
        "campaign_id": campaign_id
    })))
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
    use crate::auth::jwt::encode_token_with_role;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";
    const TEST_DB_URL: &str = "postgresql://ws@localhost:5432/kbr_test";

    async fn get_state() -> AppState {
        let pool = sqlx::PgPool::connect(TEST_DB_URL)
            .await
            .expect("Failed to connect to test database");

        let config = crate::services::storage_service::S3Config::from_env()
            .unwrap_or_else(|_| crate::services::storage_service::S3Config {
                access_key: "test".to_string(),
                secret_key: "test".to_string(),
                endpoint: "https://test.test".to_string(),
                bucket_name: "test".to_string(),
                region: "us-east-1".to_string(),
            });
        let s3 = crate::services::storage_service::create_s3_bucket(&config).unwrap_or_else(|_| {
            let creds = s3::creds::Credentials::new(Some("test"), Some("test"), None, None, None).unwrap();
            s3::bucket::Bucket::new("test", s3::region::Region::Custom { region: "us-east-1".to_string(), endpoint: "https://test.test".to_string() }, creds)
                .unwrap()
                .with_path_style()
        });

        AppState { db: pool, s3 }
    }

    fn admin_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    fn artist_token() -> String {
        encode_token_with_role(2, TEST_SECRET, 2, Some("artist".to_string())).unwrap()
    }

    async fn seed_user() -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO users (email, password_digest, role, created_at, updated_at)
               VALUES ($1, $2, $3, NOW(), NOW())
               RETURNING id",
        )
        .bind(format!("campaign_test_user_{}_{}@test.com", pid, ts))
        .bind("hashed_password_test".to_string())
        .bind(Some("artist".to_string()))
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed user")
    }

    async fn seed_artist(user_id: i64) -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO artists (name, genre, bio, user_id, prospect, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               RETURNING id",
        )
        .bind(format!("Campaign Test Artist {}_{}", pid, ts))
        .bind(Some("Electronic".to_string()))
        .bind(Some("A test artist for campaigns".to_string()))
        .bind(Some(user_id))
        .bind(Some(false))
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed artist")
    }

    async fn seed_campaign(artist_id: i64) -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO campaigns (artist_id, name, active, vinyl_sold_count, progress, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               RETURNING id",
        )
        .bind(artist_id)
        .bind(format!("Campaign Test {}_{}", pid, ts))
        .bind(true)
        .bind(25_i32)
        .bind(50_i32)
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed campaign")
    }

    async fn cleanup_campaign(campaign_id: i64) {
        let _ = sqlx::query(r"DELETE FROM campaign_pages WHERE campaign_id = $1")
            .bind(campaign_id)
            .execute(&get_state().await.db)
            .await;
        let _ = sqlx::query(r"DELETE FROM campaigns WHERE id = $1")
            .bind(campaign_id)
            .execute(&get_state().await.db)
            .await;
    }

    async fn cleanup_artist(artist_id: i64) {
        let campaigns: Vec<i64> = sqlx::query_scalar(
            r"SELECT id FROM campaigns WHERE artist_id = $1",
        )
        .bind(artist_id)
        .fetch_all(&get_state().await.db)
        .await
        .unwrap_or_default();
        for cid in campaigns {
            cleanup_campaign(cid).await;
        }
        let _ = sqlx::query(r"DELETE FROM artists WHERE id = $1")
            .bind(artist_id)
            .execute(&get_state().await.db)
            .await;
    }

    async fn cleanup_user(user_id: i64) {
        let artists: Vec<i64> = sqlx::query_scalar(
            r"SELECT id FROM artists WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&get_state().await.db)
        .await
        .unwrap_or_default();
        for aid in artists {
            cleanup_artist(aid).await;
        }
        let _ = sqlx::query(r"DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&get_state().await.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_index_public() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get().uri("/v1/campaigns").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_show_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;
        let campaign_id = seed_campaign(artist_id).await;

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/campaign/{}", campaign_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], campaign_id);

        cleanup_campaign(campaign_id).await;
        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_show_not_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM campaigns",
        )
        .fetch_one(&state.db)
        .await
        .expect("Failed to get max id");

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/campaign/{}", max_id + 9999))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_create_admin() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        let name = format!("New Campaign {}_{}", pid, ts);

        let req = test::TestRequest::post()
            .uri("/v1/campaigns")
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

        let _ = sqlx::query(r"DELETE FROM campaigns WHERE name = $1")
            .bind(&name)
            .execute(&state.db)
            .await;
        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_create_empty_name() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/campaigns")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "artist_id": artist_id,
                "name": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_index_by_user_admin() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/campaigns_by_user")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_update_success() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;
        let campaign_id = seed_campaign(artist_id).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/v1/campaign/{}", campaign_id))
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
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

        cleanup_campaign(campaign_id).await;
        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_update_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM campaigns",
        )
        .fetch_one(&state.db)
        .await
        .expect("Failed to get max id");

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/v1/campaign/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .set_json(serde_json::json!({
                "name": "Updated"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_destroy_success() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;
        let campaign_id = seed_campaign(artist_id).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/v1/campaign/{}", campaign_id))
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
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

        cleanup_campaign(campaign_id).await;
        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_destroy_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM campaigns",
        )
        .fetch_one(&state.db)
        .await
        .expect("Failed to get max id");

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/v1/campaign/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_activate_success() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        let campaign_id: i64 = sqlx::query_scalar(
            r"INSERT INTO campaigns (artist_id, name, active, vinyl_sold_count, progress, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               RETURNING id",
        )
        .bind(artist_id)
        .bind(format!("Inactive Campaign {}_{}", pid, ts))
        .bind(false)
        .bind(0_i32)
        .bind(0_i32)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed inactive campaign");

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/activate_campaign")
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .set_json(serde_json::json!({
                "campaign_id": campaign_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let active: bool = sqlx::query_scalar(
            r"SELECT active FROM campaigns WHERE id = $1",
        )
        .bind(campaign_id)
        .fetch_one(&state.db)
        .await
        .expect("Failed to check activation");
        assert!(active);

        cleanup_campaign(campaign_id).await;
        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_create_forbidden() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let token = encode_token_with_role(3, TEST_SECRET, 3, Some("user".to_string())).unwrap();
        let state = web::Data::new(get_state().await);

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/campaigns")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "artist_id": 1,
                "name": "Should Fail"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn campaigns_active_only() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let pid = std::process::id();
        let active_id: i64 = sqlx::query_scalar(
            r"INSERT INTO campaigns (artist_id, name, active, created_at, updated_at)
               VALUES ($1, $2, true, NOW(), NOW())
               RETURNING id",
        )
        .bind(artist_id)
        .bind(format!("Active Only {}_{}", pid, ts))
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed active campaign");

        let inactive_id: i64 = sqlx::query_scalar(
            r"INSERT INTO campaigns (artist_id, name, active, created_at, updated_at)
               VALUES ($1, $2, false, NOW(), NOW())
               RETURNING id",
        )
        .bind(artist_id)
        .bind(format!("Inactive Only {}_{}", pid, ts))
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed inactive campaign");

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get().uri("/v1/active_campaigns").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        let arr = body.as_array().unwrap();
        let ids: Vec<i64> = arr.iter().map(|v| v["id"].as_i64().unwrap()).collect();
        assert!(ids.contains(&active_id));
        assert!(!ids.contains(&inactive_id));

        cleanup_campaign(active_id).await;
        cleanup_campaign(inactive_id).await;
        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }
}
