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
use chrono::NaiveDateTime;
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::news::{CreateNewsRequest, News, NewsResponse, UpdateNewsRequest};
use crate::services::og_tags::OgTagsService;

#[derive(Debug, FromRow)]
struct NewsRow {
    id: i64,
    url: Option<String>,
    title: Option<String>,
    vote_score: Option<i32>,
    flagged: Option<bool>,
    flagged_at: Option<NaiveDateTime>,
    user_id: i64,
    image_url: Option<String>,
    active: Option<bool>,
    comments_enabled: bool,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

impl From<NewsRow> for News {
    fn from(row: NewsRow) -> Self {
        News {
            id: row.id,
            url: row.url,
            title: row.title,
            vote_score: row.vote_score,
            flagged: row.flagged,
            flagged_at: row.flagged_at.map(|t| t.and_utc()),
            user_id: row.user_id,
            image_url: row.image_url,
            active: row.active,
            comments_enabled: row.comments_enabled,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const NEWS_COLUMNS: &str =
    "id, url, title, vote_score, flagged, flagged_at, user_id, image_url, active, comments_enabled, created_at, updated_at";

/// List all news articles.
///
/// Returns all news articles ordered by ID. No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `NewsResponse`
pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let rows = sqlx::query_as::<_, NewsRow>(
        &format!(r"SELECT {} FROM news ORDER BY id", NEWS_COLUMNS),
    )
    .fetch_all(&state.db)
    .await?;

    let news: Vec<News> = rows.into_iter().map(|r| r.into()).collect();
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

    match sqlx::query_as::<_, NewsRow>(
        &format!(r"SELECT {} FROM news WHERE id = $1", NEWS_COLUMNS),
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let news: News = row.into();
            Ok(HttpResponse::Ok().json(news.to_response()))
        }
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

    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM news WHERE url = $1",
    )
    .bind(&body.url)
    .fetch_optional(&state.db)
    .await?;

    if let Some((existing_id,)) = existing {
        let row = sqlx::query_as::<_, NewsRow>(
            &format!(r"SELECT {} FROM news WHERE id = $1", NEWS_COLUMNS),
        )
        .bind(existing_id)
        .fetch_one(&state.db)
        .await?;
        let news: News = row.into();
        return Ok(HttpResponse::Conflict().json(serde_json::json!({
            "error": "news url already exists",
            "data": news.to_response()
        })));
    }

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, NewsRow>(
        &format!(
            r"INSERT INTO news (url, title, user_id, image_url, active, comments_enabled, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING {}",
            NEWS_COLUMNS
        ),
    )
    .bind(&body.url)
    .bind(title)
    .bind(user.id)
    .bind(image_url)
    .bind(true)
    .bind(true)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let news: News = row.into();
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

    let existing = sqlx::query_as::<_, NewsRow>(
        &format!(r"SELECT {} FROM news WHERE id = $1", NEWS_COLUMNS),
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let news = match existing {
        Some(row) => News::from(row),
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

    let (row, new_active, new_comments) = if let Some(active) = active_opt {
        if let Some(enabled) = comments_opt {
            let r = sqlx::query_as::<_, NewsRow>(
                &format!(
                    r"UPDATE news SET active = $1, comments_enabled = $2, updated_at = $3 WHERE id = $4 RETURNING {}",
                    NEWS_COLUMNS
                ),
            )
            .bind(active)
            .bind(enabled)
            .bind(now)
            .bind(id)
            .fetch_one(&state.db)
            .await?;
            (r, Some(active), Some(enabled))
        } else {
            let r = sqlx::query_as::<_, NewsRow>(
                &format!(
                    r"UPDATE news SET active = $1, updated_at = $2 WHERE id = $3 RETURNING {}",
                    NEWS_COLUMNS
                ),
            )
            .bind(active)
            .bind(now)
            .bind(id)
            .fetch_one(&state.db)
            .await?;
            (r, Some(active), None)
        }
    } else if let Some(enabled) = comments_opt {
        let r = sqlx::query_as::<_, NewsRow>(
            &format!(
                r"UPDATE news SET comments_enabled = $1, updated_at = $2 WHERE id = $3 RETURNING {}",
                NEWS_COLUMNS
            ),
        )
        .bind(enabled)
        .bind(now)
        .bind(id)
        .fetch_one(&state.db)
        .await?;
        (r, None, Some(enabled))
    } else {
        unreachable!();
    };

    let mut news: News = row.into();
    if let Some(a) = new_active {
        news.active = Some(a);
    }
    if let Some(c) = new_comments {
        news.comments_enabled = c;
    }
    Ok(HttpResponse::Ok().json(news.to_response()))
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

    let existing = sqlx::query_as::<_, NewsRow>(
        &format!(r"SELECT {} FROM news WHERE id = $1", NEWS_COLUMNS),
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let news = match existing {
        Some(row) => News::from(row),
        None => return Err(AppError::NotFound(format!("News #{}", id))),
    };

    if news.user_id != user.id && !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    let now = chrono::Utc::now().naive_utc();
    let row = sqlx::query_as::<_, NewsRow>(
        &format!(
            r"UPDATE news SET comments_enabled = $1, updated_at = $2 WHERE id = $3 RETURNING {}",
            NEWS_COLUMNS
        ),
    )
    .bind(!news.comments_enabled)
    .bind(now)
    .bind(id)
    .fetch_one(&state.db)
    .await?;

    let news: News = row.into();
    Ok(HttpResponse::Ok().json(news.to_response()))
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

    let news_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM news WHERE id = $1)",
    )
    .bind(news_id)
    .fetch_one(&state.db)
    .await?;
    if !news_exists {
        return Err(AppError::NotFound(format!("News #{}", news_id)));
    }

    let playlist: Option<(i64, i64)> = sqlx::query_as(
        "SELECT id, user_id FROM news_playlists WHERE id = $1",
    )
    .bind(playlist_id)
    .fetch_optional(&state.db)
    .await?;

    let (_playlist_id, playlist_user_id) = match playlist {
        Some(row) => row,
        None => return Err(AppError::NotFound(format!("Playlist #{}", playlist_id))),
    };

    if playlist_user_id != user.id {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    let existing: Option<(i64,)> = sqlx::query_as(
        "SELECT id FROM users_news WHERE playlist_id = $1 AND news_id = $2",
    )
    .bind(playlist_id)
    .bind(news_id)
    .fetch_optional(&state.db)
    .await?;

    if existing.is_some() {
        return Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("News #{} already in playlist #{}", news_id, playlist_id),
            "user_id": user.id
        })));
    }

    let now = chrono::Utc::now().naive_utc();

    let max_position: Option<i32> = sqlx::query_scalar(
        r"SELECT COALESCE(MAX(position), -1) FROM users_news WHERE playlist_id = $1",
    )
    .bind(playlist_id)
    .fetch_optional(&state.db)
    .await?;

    let position = max_position.map(|p| p + 1).unwrap_or(0);

    let _ = sqlx::query(
        r"INSERT INTO users_news (user_id, news_id, playlist_id, position, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user.id)
    .bind(news_id)
    .bind(playlist_id)
    .bind(position)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

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
    use crate::auth::jwt::encode_token_with_role;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";
    const TEST_DB_URL: &str = "postgresql://ws@localhost:5432/kbr_test";

    async fn get_state() -> AppState {
        let pool = sqlx::PgPool::connect(TEST_DB_URL)
            .await
            .expect("Failed to connect to test database");
        crate::test_utils::build_test_state(pool).await
    }

    fn admin_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap()
    }

    fn user_token(user_id: i64) -> String {
        encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap()
    }

    async fn seed_news(state: &AppState, suffix: &str) -> i64 {
        let row = sqlx::query_as::<_, NewsRow>(
            &format!(
                r"INSERT INTO news (url, title, user_id, active, comments_enabled, created_at, updated_at)
                 VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
                 RETURNING {}",
                NEWS_COLUMNS
            ),
        )
        .bind(format!("https://example.com/test-{}", suffix))
        .bind(format!("Test News {}", suffix))
        .bind(1i64)
        .bind(true)
        .bind(true)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed news");
        row.id
    }

    async fn cleanup_news(state: &AppState, title: &str) {
        let _ = sqlx::query(r"DELETE FROM users_news WHERE news_id IN (SELECT id FROM news WHERE title = $1)")
            .bind(title)
            .execute(&state.db)
            .await;
        let _ = sqlx::query(r"DELETE FROM news WHERE title = $1")
            .bind(title)
            .execute(&state.db)
            .await;
    }

    async fn seed_playlist(state: &AppState, suffix: &str) -> i64 {
        let row: (i64,) = sqlx::query_as(
            r"INSERT INTO news_playlists (user_id, name, description, created_at, updated_at)
              VALUES ($1, $2, $3, NOW(), NOW())
              RETURNING id",
        )
        .bind(1i64)
        .bind(format!("Test Playlist {}", suffix))
        .bind(format!("Playlist desc {}", suffix))
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed playlist");
        row.0
    }

    async fn cleanup_playlist(state: &AppState, name: &str) {
        let _ = sqlx::query(r"DELETE FROM users_news WHERE playlist_id IN (SELECT id FROM news_playlists WHERE name = $1)")
            .bind(name)
            .execute(&state.db)
            .await;
        let _ = sqlx::query(r"DELETE FROM news_playlists WHERE name = $1")
            .bind(name)
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_index_returns_ok() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get().uri("/news").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_show_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let title = format!("Test News {}", suffix);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/news/{}", news_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["title"], title);

        cleanup_news(&state, &title).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_show_not_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(r"SELECT COALESCE(MAX(id), 0) FROM news")
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
            .uri(&format!("/news/{}", max_id + 9999))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_create_valid_url() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

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

        cleanup_news(&state, &format!("Created News {}", suffix)).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_create_malicious_url() {
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

        let req = test::TestRequest::post()
            .uri("/news")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "url": "http://localhost/admin"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_update_active() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let title = format!("Test News {}", suffix);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

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

        cleanup_news(&state, &title).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_update_forbidden() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let title = format!("Test News {}", suffix);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/news/{}", news_id))
            .insert_header(("Authorization", format!("Bearer {}", user_token(9999))))
            .set_json(serde_json::json!({
                "active": false
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        cleanup_news(&state, &title).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_toggle_comments() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let title = format!("Test News {}", suffix);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/news/{}/toggle_comments", news_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["comments_enabled"], false);

        cleanup_news(&state, &title).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_add_to_playlist() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let news_title = format!("Test News {}", suffix);
        let playlist_id = seed_playlist(&state, &suffix).await;
        let playlist_name = format!("Test Playlist {}", suffix);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

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

        cleanup_news(&state, &news_title).await;
        cleanup_playlist(&state, &playlist_name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_create_duplicate_url_returns_409() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let url = format!("https://example.com/dup-{}", suffix);
        let title = format!("Dup News {}", suffix);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

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

        cleanup_news(&state, &title).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_add_to_playlist_forbidden_wrong_owner() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let news_title = format!("Test News {}", suffix);
        let playlist_id = seed_playlist(&state, &suffix).await;
        let playlist_name = format!("Test Playlist {}", suffix);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

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

        cleanup_news(&state, &news_title).await;
        cleanup_playlist(&state, &playlist_name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_add_to_playlist_news_not_found() {
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
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_add_to_playlist_idempotent() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let suffix = format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos());
        let news_id = seed_news(&state, &suffix).await;
        let news_title = format!("Test News {}", suffix);
        let playlist_id = seed_playlist(&state, &suffix).await;
        let playlist_name = format!("Test Playlist {}", suffix);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

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

        cleanup_news(&state, &news_title).await;
        cleanup_playlist(&state, &playlist_name).await;
    }
}
