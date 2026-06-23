//! News handlers
//!
//! Provides endpoints for news article management including creation, updates,
//! comment toggling, and playlist integration. Read endpoints are public;
//! write operations require authentication with ownership or admin verification.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/news` | public | List all news articles |
//! | `show` | GET | `/v1/news/{id}` | public | Retrieve a single news article by ID |
//! | `create` | POST | `/v1/news` | auth | Create a new news article |
//! | `update` | PUT | `/v1/news/{id}` | auth | Update news active/comments status |
//! | `toggle_comments` | POST | `/v1/news/{id}/toggle_comments` | auth | Toggle comments on/off for a news article |
//! | `add_to_playlist` | POST | `/v1/news/add_to_playlist` | auth | Add a news article to a playlist |

use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::data::news as data;
use crate::error::AppError;
use crate::models::news::{CreateNewsRequest, News, NewsResponse, UpdateNewsRequest};
use crate::services::og_tags::OgTagsService;

/// List all news articles.
///
/// Returns all news articles ordered by ID. No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `NewsResponse`
pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let news = data::list(&state.db).await?;
    let responses: Vec<NewsResponse> = news.iter().map(|n| n.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Retrieve a single news article by ID.
///
/// No authentication required.
///
/// # Response
///
/// `200 OK` — `NewsResponse`
/// `404 Not Found` — news article does not exist
pub async fn show(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    match data::by_id(&state.db, id).await? {
        Some(news) => Ok(HttpResponse::Ok().json(news.to_response())),
        None => Err(AppError::NotFound(format!("News #{}", id))),
    }
}

/// Create a new news article.
///
/// Requires authentication. Validates URL format and checks against Google
/// Safe Browsing if configured. Fetches Open Graph tags to populate image_url
/// and title (fallback). Checks for duplicate URLs before inserting.
/// The creating user is recorded as the article owner.
///
/// # Response
///
/// `201 Created` — `NewsResponse`
/// `409 Conflict` — URL already exists
/// `422 Unprocessable` — URL is invalid or malicious
pub async fn create(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<CreateNewsRequest>,
) -> Result<HttpResponse, AppError> {
    if !News::validate_url(&body.url) {
        return Err(AppError::Validation("Invalid or unsafe URL".to_string()));
    }

    if let Some(ref sb) = state.safe_browsing
        && sb.is_malicious(&body.url).await
    {
        return Err(AppError::UnprocessableEntity("The URL provided is malicious".to_string()));
    }

    let og = OgTagsService::new();
    let og_tags = og.fetch(&body.url).await;

    let image_url = og_tags.as_ref().and_then(|t| t.get("image").cloned());
    let title = body.title.clone().or_else(|| og_tags.and_then(|t| t.get("title").cloned()));

    if let Some(existing_news) = data::exists_by_url(&state.db, &body.url).await? {
        return Ok(HttpResponse::Conflict().json(serde_json::json!({
            "error": "news url already exists",
            "data": existing_news.to_response()
        })));
    }

    let now = chrono::Utc::now().naive_utc();

    let news = data::create(&state.db, &body.url, &title, user.id, &image_url, true, true, now).await?;
    Ok(HttpResponse::Created().json(news.to_response()))
}

/// Update a news article's active status and/or comments setting.
///
/// Requires authentication. Only the article owner or an admin can update.
/// Supports partial updates of `active` and `comments_enabled` fields.
///
/// # Response
///
/// `200 OK` — `NewsResponse`
/// `403 Forbidden` — user is not the owner or admin
/// `404 Not Found` — news article does not exist
pub async fn update(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateNewsRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    let existing = data::by_id(&state.db, id).await?;

    let news = match existing {
        Some(n) => n,
        None => return Err(AppError::NotFound(format!("News #{}", id))),
    };

    if news.user_id != user.id && !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    let active_opt = body.active;
    let comments_opt = body.comments_enabled;

    if active_opt.is_none() && comments_opt.is_none() {
        return Ok(HttpResponse::Ok().json(news.to_response()));
    }

    let now = chrono::Utc::now().naive_utc();

    match data::update(&state.db, id, active_opt, comments_opt, now).await? {
        Some(row) => {
            let mut updated: News = row.into();
            if let Some(a) = active_opt {
                updated.active = Some(a);
            }
            if let Some(c) = comments_opt {
                updated.comments_enabled = c;
            }
            Ok(HttpResponse::Ok().json(updated.to_response()))
        }
        None => Err(AppError::NotFound(format!("News #{}", id))),
    }
}

/// Toggle comments on/off for a news article.
///
/// Requires authentication. Only the article owner or an admin can toggle.
/// Flips the `comments_enabled` boolean.
///
/// # Response
///
/// `200 OK` — `NewsResponse` with updated comments_enabled value
/// `403 Forbidden` — user is not the owner or admin
/// `404 Not Found` — news article does not exist
pub async fn toggle_comments(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    let existing = data::by_id(&state.db, id).await?;

    let news = match existing {
        Some(n) => n,
        None => return Err(AppError::NotFound(format!("News #{}", id))),
    };

    if news.user_id != user.id && !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    let now = chrono::Utc::now().naive_utc();
    let updated = data::toggle_comments(&state.db, id, !news.comments_enabled, now).await?;
    Ok(HttpResponse::Ok().json(updated.to_response()))
}

/// Add a news article to a playlist.
///
/// Requires authentication. Request body must contain `news_id` and `playlist_id`.
/// Verifies both the news article and playlist exist, and that the playlist
/// belongs to the authenticated user. Idempotent: returns existing entry if
/// the news is already in the playlist.
///
/// # Response
///
/// `200 OK` — JSON object with confirmation message and user ID
/// `403 Forbidden` — playlist does not belong to the user
/// `404 Not Found` — news or playlist does not exist
/// `422 Unprocessable` — missing news_id or playlist_id
pub async fn add_to_playlist(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let news_id = body.get("news_id").and_then(|v| v.as_i64()).ok_or_else(|| {
        AppError::Validation("news_id is required".to_string())
    })?;
    let playlist_id = body.get("playlist_id").and_then(|v| v.as_i64()).ok_or_else(|| {
        AppError::Validation("playlist_id is required".to_string())
    })?;

    if !data::news_exists(&state.db, news_id).await? {
        return Err(AppError::NotFound(format!("News #{}", news_id)));
    }

    let playlist = data::get_playlist(&state.db, playlist_id).await?;

    let (_playlist_id, playlist_user_id) = match playlist {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Playlist #{}", playlist_id))),
    };

    if playlist_user_id != user.id {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    if data::playlist_has_news(&state.db, playlist_id, news_id).await? {
        return Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("News #{} already in playlist #{}", news_id, playlist_id),
            "user_id": user.id
        })));
    }

    let now = chrono::Utc::now().naive_utc();

    let max_position = data::max_position_in_playlist(&state.db, playlist_id).await?.unwrap_or(-1);
    let position = max_position + 1;

    data::add_news_to_playlist(&state.db, user.id, news_id, playlist_id, position, now).await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("News #{} added to playlist #{}", news_id, playlist_id),
        "user_id": user.id
    })))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/news", web::get().to(index))
        .route("/news/{id}", web::get().to(show))
        .route("/news", web::post().to(create))
        .route("/news/{id}", web::put().to(update))
        .route("/news/{id}/toggle_comments", web::post().to(toggle_comments))
        .route("/news/add_to_playlist", web::post().to(add_to_playlist));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{admin_token, not_found_id, seed_news_playlist, seed_user_with_id, user_token};
    use actix_web::test;

    async fn seed_news(state: &AppState, suffix: &str) -> i64 {
        seed_user_with_id(&state.db, 1, "admin@test.com", "admin").await;
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO news (url, title, user_id, active, comments_enabled, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
             RETURNING id",
        )
        .bind(format!("https://example.com/test-{}", suffix))
        .bind(format!("Test News {}", suffix))
        .bind(1i64)
        .bind(true)
        .bind(true)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed news")
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_index_returns_ok() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get().uri("/news").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_show_found() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let title = format!("Test News {}", suffix);

        let req = test::TestRequest::get()
            .uri(&format!("/news/{}", news_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["title"], title);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_show_not_found() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "news").await;

        let req = test::TestRequest::get()
            .uri(&format!("/news/{}", not_found))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_create_valid_url() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        seed_user_with_id(&state.db, 1, "admin@test.com", "admin").await;

        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let req = test::TestRequest::post()
            .uri("/news")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "url": format!("https://example.com/article-{}", suffix),
                "title": format!("Created News {}", suffix)
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["title"], format!("Created News {}", suffix));

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_create_malicious_url() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/news")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "url": "http://localhost/admin"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_update_active() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let _title = format!("Test News {}", suffix);

        let req = test::TestRequest::put()
            .uri(&format!("/news/{}", news_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "active": false
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["active"], false);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_update_forbidden() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let _title = format!("Test News {}", suffix);

        let req = test::TestRequest::put()
            .uri(&format!("/news/{}", news_id))
            .insert_header(("Authorization", format!("Bearer {}", user_token(9999))))
            .set_json(serde_json::json!({
                "active": false
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_toggle_comments() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let _title = format!("Test News {}", suffix);

        let req = test::TestRequest::post()
            .uri(&format!("/news/{}/toggle_comments", news_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["comments_enabled"], false);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_add_to_playlist() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let _news_title = format!("Test News {}", suffix);
        let playlist_name = format!("Test Playlist {}", suffix);
        let playlist_id = seed_news_playlist(&state.db, 1, &playlist_name).await;

        let req = test::TestRequest::post()
            .uri("/news/add_to_playlist")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "news_id": news_id,
                "playlist_id": playlist_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["message"].as_str().unwrap().contains(&news_id.to_string()));

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_create_duplicate_url_returns_409() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        seed_user_with_id(&state.db, 1, "admin@test.com", "admin").await;
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let url = format!("https://example.com/dup-{}", suffix);
        let title = format!("Dup News {}", suffix);

        let req = test::TestRequest::post()
            .uri("/news")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "url": url.clone(),
                "title": title.clone()
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let req = test::TestRequest::post()
            .uri("/news")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "url": url.clone(),
                "title": "Different Title"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 409);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["error"], "news url already exists");
        assert_eq!(body["data"]["title"], title);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_add_to_playlist_forbidden_wrong_owner() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let _news_title = format!("Test News {}", suffix);
        let playlist_name = format!("Test Playlist {}", suffix);
        let playlist_id = seed_news_playlist(&state.db, 1, &playlist_name).await;

        let req = test::TestRequest::post()
            .uri("/news/add_to_playlist")
            .insert_header(("Authorization", format!("Bearer {}", user_token(9999))))
            .set_json(serde_json::json!({
                "news_id": news_id,
                "playlist_id": playlist_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_add_to_playlist_news_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/news/add_to_playlist")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "news_id": 99999999,
                "playlist_id": 1
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_add_to_playlist_idempotent() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let _news_title = format!("Test News {}", suffix);
        let playlist_name = format!("Test Playlist {}", suffix);
        let playlist_id = seed_news_playlist(&state.db, 1, &playlist_name).await;

        let req = test::TestRequest::post()
            .uri("/news/add_to_playlist")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "news_id": news_id,
                "playlist_id": playlist_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let req = test::TestRequest::post()
            .uri("/news/add_to_playlist")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "news_id": news_id,
                "playlist_id": playlist_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["message"].as_str().unwrap().contains("already in playlist"));

        _guard.cleanup().await;
    }
}
