use actix_web::{web, HttpResponse};
use chrono::NaiveDateTime;
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::news_playlist::{
    CreateNewsPlaylistRequest, NewsPlaylist, NewsPlaylistResponse, UpdateNewsPlaylistRequest,
};

#[derive(Debug, FromRow)]
struct PlaylistRow {
    id: i64,
    user_id: i64,
    name: String,
    description: Option<String>,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
}

impl From<PlaylistRow> for NewsPlaylist {
    fn from(row: PlaylistRow) -> Self {
        NewsPlaylist {
            id: row.id,
            user_id: row.user_id,
            name: row.name,
            description: row.description,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const PLAYLIST_COLUMNS: &str = "id, user_id, name, description, created_at, updated_at";

pub async fn index_admin(
    state: web::Data<AppState>,
    user: CurrentUser,
    query: web::Query<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    let user_id = query.0.get("user_id").and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok())));

    let rows = if let Some(uid) = user_id {
        sqlx::query_as::<_, PlaylistRow>(
            &format!(r"SELECT {} FROM news_playlists WHERE user_id = $1 ORDER BY id", PLAYLIST_COLUMNS),
        )
        .bind(uid)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, PlaylistRow>(
            &format!(r"SELECT {} FROM news_playlists ORDER BY id", PLAYLIST_COLUMNS),
        )
        .fetch_all(&state.db)
        .await?
    };

    let playlists: Vec<NewsPlaylist> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<NewsPlaylistResponse> = playlists.iter().map(|p| p.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show_admin(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();

    match sqlx::query_as::<_, PlaylistRow>(
        &format!(r"SELECT {} FROM news_playlists WHERE id = $1", PLAYLIST_COLUMNS),
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let playlist: NewsPlaylist = row.into();
            Ok(HttpResponse::Ok().json(playlist.to_response()))
        }
        None => Err(AppError::NotFound(format!("Playlist #{}", id))),
    }
}

pub async fn destroy_admin(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();

    match sqlx::query_as::<_, PlaylistRow>(
        &format!(r"SELECT {} FROM news_playlists WHERE id = $1", PLAYLIST_COLUMNS),
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(_) => {
            let _ = sqlx::query(r"DELETE FROM users_news WHERE playlist_id = $1")
                .bind(id)
                .execute(&state.db)
                .await;
            let _ = sqlx::query(r"DELETE FROM news_playlists WHERE id = $1")
                .bind(id)
                .execute(&state.db)
                .await;
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "message": format!("Playlist #{} deleted", id)
            })))
        }
        None => Err(AppError::NotFound(format!("Playlist #{}", id))),
    }
}

pub async fn dashboard_index(
    state: web::Data<AppState>,
    user: CurrentUser,
) -> Result<HttpResponse, AppError> {
    let rows = sqlx::query_as::<_, PlaylistRow>(
        &format!(r"SELECT {} FROM news_playlists WHERE user_id = $1 ORDER BY id", PLAYLIST_COLUMNS),
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let playlists: Vec<NewsPlaylist> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<NewsPlaylistResponse> = playlists.iter().map(|p| p.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn dashboard_show(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    match sqlx::query_as::<_, PlaylistRow>(
        &format!(r"SELECT {} FROM news_playlists WHERE id = $1", PLAYLIST_COLUMNS),
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let playlist: NewsPlaylist = row.into();
            if playlist.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
            Ok(HttpResponse::Ok().json(playlist.to_response()))
        }
        None => Err(AppError::NotFound(format!("Playlist #{}", id))),
    }
}

pub async fn dashboard_create(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<CreateNewsPlaylistRequest>,
) -> Result<HttpResponse, AppError> {
    if body.name.is_empty() {
        return Err(AppError::Validation("Playlist name is required".to_string()));
    }

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, PlaylistRow>(
        &format!(
            r"INSERT INTO news_playlists (user_id, name, description, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5)
             RETURNING {}",
            PLAYLIST_COLUMNS
        ),
    )
    .bind(user.id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let playlist: NewsPlaylist = row.into();
    Ok(HttpResponse::Created().json(playlist.to_response()))
}

pub async fn dashboard_update(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateNewsPlaylistRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    let existing = sqlx::query_as::<_, PlaylistRow>(
        &format!(r"SELECT {} FROM news_playlists WHERE id = $1", PLAYLIST_COLUMNS),
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let playlist = match existing {
        Some(row) => NewsPlaylist::from(row),
        None => return Err(AppError::NotFound(format!("Playlist #{}", id))),
    };

    if playlist.user_id != user.id && !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    let name_opt = body.name.clone();
    let desc_opt = body.description.clone();

    if name_opt.is_none() && desc_opt.is_none() {
        return Ok(HttpResponse::Ok().json(playlist.to_response()));
    }

    let now = chrono::Utc::now().naive_utc();

    let row = if let Some(name) = name_opt {
        if let Some(desc) = desc_opt {
            sqlx::query_as::<_, PlaylistRow>(
                &format!(
                    r"UPDATE news_playlists SET name = $1, description = $2, updated_at = $3 WHERE id = $4 RETURNING {}",
                    PLAYLIST_COLUMNS
                ),
            )
            .bind(&name)
            .bind(&desc)
            .bind(now)
            .bind(id)
            .fetch_one(&state.db)
            .await?
        } else {
            sqlx::query_as::<_, PlaylistRow>(
                &format!(
                    r"UPDATE news_playlists SET name = $1, updated_at = $2 WHERE id = $3 RETURNING {}",
                    PLAYLIST_COLUMNS
                ),
            )
            .bind(&name)
            .bind(now)
            .bind(id)
            .fetch_one(&state.db)
            .await?
        }
    } else {
        sqlx::query_as::<_, PlaylistRow>(
            &format!(
                r"UPDATE news_playlists SET description = $1, updated_at = $2 WHERE id = $3 RETURNING {}",
                PLAYLIST_COLUMNS
            ),
        )
        .bind(&desc_opt)
        .bind(now)
        .bind(id)
        .fetch_one(&state.db)
        .await?
    };

    let playlist: NewsPlaylist = row.into();
    Ok(HttpResponse::Ok().json(playlist.to_response()))
}

pub async fn dashboard_destroy(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    let existing = sqlx::query_as::<_, PlaylistRow>(
        &format!(r"SELECT {} FROM news_playlists WHERE id = $1", PLAYLIST_COLUMNS),
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    let playlist = match existing {
        Some(row) => NewsPlaylist::from(row),
        None => return Err(AppError::NotFound(format!("Playlist #{}", id))),
    };

    if playlist.user_id != user.id && !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }

    let _ = sqlx::query(r"DELETE FROM users_news WHERE playlist_id = $1")
        .bind(id)
        .execute(&state.db)
        .await;
    let _ = sqlx::query(r"DELETE FROM news_playlists WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Playlist #{} deleted", id)
    })))
}

pub async fn dashboard_add_news(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let playlist_id = path.into_inner();

    let playlist = sqlx::query_as::<_, PlaylistRow>(
        &format!(r"SELECT {} FROM news_playlists WHERE id = $1", PLAYLIST_COLUMNS),
    )
    .bind(playlist_id)
    .fetch_optional(&state.db)
    .await?;

    match playlist {
        Some(row) => {
            let p: NewsPlaylist = row.into();
            if p.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
        }
        None => return Err(AppError::NotFound(format!("Playlist #{}", playlist_id))),
    }

    let news_id = body.get("news_id").and_then(|v| v.as_i64()).ok_or_else(|| {
        AppError::Validation("news_id is required".to_string())
    })?;

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
        "message": format!("News #{} added to playlist #{}", news_id, playlist_id)
    })))
}

pub async fn dashboard_reorder(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let playlist_id = path.into_inner();

    let playlist = sqlx::query_as::<_, PlaylistRow>(
        &format!(r"SELECT {} FROM news_playlists WHERE id = $1", PLAYLIST_COLUMNS),
    )
    .bind(playlist_id)
    .fetch_optional(&state.db)
    .await?;

    match playlist {
        Some(row) => {
            let p: NewsPlaylist = row.into();
            if p.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
        }
        None => return Err(AppError::NotFound(format!("Playlist #{}", playlist_id))),
    }

    let news_ids = body.get("news_ids").and_then(|v| v.as_array()).ok_or_else(|| {
        AppError::Validation("news_ids array is required".to_string())
    })?;

    let now = chrono::Utc::now().naive_utc();

    for (position, item) in news_ids.iter().enumerate() {
        if let Some(news_id) = item.as_i64() {
            let _ = sqlx::query(
                r"UPDATE users_news SET position = $1, updated_at = $2 WHERE playlist_id = $3 AND news_id = $4",
            )
            .bind(position as i32)
            .bind(now)
            .bind(playlist_id)
            .bind(news_id)
            .execute(&state.db)
            .await;
        }
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Playlist reordered"
    })))
}

pub async fn dashboard_remove_news(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    let (playlist_id, news_id) = path.into_inner();

    let playlist = sqlx::query_as::<_, PlaylistRow>(
        &format!(r"SELECT {} FROM news_playlists WHERE id = $1", PLAYLIST_COLUMNS),
    )
    .bind(playlist_id)
    .fetch_optional(&state.db)
    .await?;

    match playlist {
        Some(row) => {
            let p: NewsPlaylist = row.into();
            if p.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
        }
        None => return Err(AppError::NotFound(format!("Playlist #{}", playlist_id))),
    }

    let _ = sqlx::query(
        r"DELETE FROM users_news WHERE playlist_id = $1 AND news_id = $2",
    )
    .bind(playlist_id)
    .bind(news_id)
    .execute(&state.db)
    .await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("News #{} removed from playlist #{}", news_id, playlist_id)
    })))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/admin/news_playlists", web::get().to(index_admin))
            .route("/admin/news_playlists/{id}", web::get().to(show_admin))
            .route("/admin/news_playlists/{id}", web::delete().to(destroy_admin))
            .route("/dashboard/news_playlists", web::get().to(dashboard_index))
            .route("/dashboard/news_playlists/{id}", web::get().to(dashboard_show))
            .route("/dashboard/news_playlists", web::post().to(dashboard_create))
            .route("/dashboard/news_playlists/{id}", web::put().to(dashboard_update))
            .route("/dashboard/news_playlists/{id}", web::delete().to(dashboard_destroy))
            .route("/dashboard/news_playlists/{id}/add_news", web::post().to(dashboard_add_news))
            .route("/dashboard/news_playlists/{id}/reorder", web::post().to(dashboard_reorder))
            .route(
                "/dashboard/news_playlists/{playlist_id}/remove_news/{news_id}",
                web::post().to(dashboard_remove_news),
            ),
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

    fn user_token(user_id: i64) -> String {
        encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string())).unwrap()
    }

    async fn seed_playlist(state: &AppState, suffix: &str) -> (i64, String) {
        let name = format!("Test Playlist {}", suffix);
        let row: (i64,) = sqlx::query_as(
            r"INSERT INTO news_playlists (user_id, name, description, created_at, updated_at)
              VALUES ($1, $2, $3, NOW(), NOW())
              RETURNING id",
        )
        .bind(1i64)
        .bind(&name)
        .bind(Some(format!("Playlist desc {}", suffix)))
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed playlist");
        (row.0, name)
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

    fn suffix() -> String {
        format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn admin_playlists_index() {
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
            .uri("/v1/admin/news_playlists")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn admin_playlist_show_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let s = suffix();
        let (playlist_id, name) = seed_playlist(&state, &s).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/admin/news_playlists/{}", playlist_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["name"], name);

        cleanup_playlist(&state, &name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn admin_playlist_show_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(r"SELECT COALESCE(MAX(id), 0) FROM news_playlists")
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
            .uri(&format!("/v1/admin/news_playlists/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn admin_playlist_destroy() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let s = suffix();
        let (playlist_id, name) = seed_playlist(&state, &s).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/v1/admin/news_playlists/{}", playlist_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let remaining: Option<i64> = sqlx::query_scalar(r"SELECT id FROM news_playlists WHERE name = $1")
            .bind(&name)
            .fetch_optional(&state.db)
            .await
            .expect("Failed to check cleanup");
        assert!(remaining.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_playlists_index() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let s = suffix();
        let (_playlist_id, name) = seed_playlist(&state, &s).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/dashboard/news_playlists")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        let found = body.as_array().unwrap().iter().any(|v| v["name"] == name);
        assert!(found, "Created playlist should appear in user's list");

        cleanup_playlist(&state, &name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_create_playlist() {
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

        let s = suffix();
        let req = test::TestRequest::post()
            .uri("/v1/dashboard/news_playlists")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "name": format!("New Playlist {}", s),
                "description": "My new playlist"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["name"], format!("New Playlist {}", s));
        assert_eq!(body["user_id"], 1);

        cleanup_playlist(&state, &format!("New Playlist {}", s)).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_create_empty_name() {
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
            .uri("/v1/dashboard/news_playlists")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "name": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_update_playlist() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let s = suffix();
        let (playlist_id, name) = seed_playlist(&state, &s).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/v1/dashboard/news_playlists/{}", playlist_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "name": format!("Updated {}", s),
                "description": "Updated description"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["name"], format!("Updated {}", s));
        assert_eq!(body["description"], "Updated description");

        cleanup_playlist(&state, &name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_update_forbidden() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let s = suffix();
        let (_playlist_id, name) = seed_playlist(&state, &s).await;

        let (playlist_id, _) = {
            let row: (i64,) = sqlx::query_as(r"SELECT id FROM news_playlists WHERE name = $1")
                .bind(&name)
                .fetch_one(&state.db)
                .await
                .expect("Failed to find playlist");
            (row.0, name.clone())
        };

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/v1/dashboard/news_playlists/{}", playlist_id))
            .insert_header(("Authorization", format!("Bearer {}", user_token(9999))))
            .set_json(serde_json::json!({
                "name": "Hacked"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        cleanup_playlist(&state, &name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_destroy_playlist() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let s = suffix();
        let (playlist_id, name) = seed_playlist(&state, &s).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/v1/dashboard/news_playlists/{}", playlist_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let remaining: Option<i64> = sqlx::query_scalar(r"SELECT id FROM news_playlists WHERE name = $1")
            .bind(&name)
            .fetch_optional(&state.db)
            .await
            .expect("Failed to check cleanup");
        assert!(remaining.is_none());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_destroy_forbidden() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let s = suffix();
        let (playlist_id, name) = seed_playlist(&state, &s).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/v1/dashboard/news_playlists/{}", playlist_id))
            .insert_header(("Authorization", format!("Bearer {}", user_token(9999))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        cleanup_playlist(&state, &name).await;
    }
}
