//! Playlist handlers
//!
//! Provides endpoints for managing playlists, including admin CRUD
//! operations and dashboard views. Admin endpoints require artist role
//! or above; dashboard endpoints require authentication.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index_admin` | GET | `/v1/admin/news_playlists` | admin | List all playlists (optionally filtered by user_id) |
//! | `show_admin` | GET | `/v1/admin/news_playlists/{id}` | admin | Retrieve a single playlist by ID |
//! | `destroy_admin` | DELETE | `/v1/admin/news_playlists/{id}` | admin | Delete a playlist and its associated users_news entries |
//! | `dashboard_index` | GET | `/v1/dashboard/news_playlists` | auth | List playlists for the authenticated user |
//! | `dashboard_show` | GET | `/v1/dashboard/news_playlists/{id}` | auth | Get a single playlist (owner or admin only) |
//! | `dashboard_create` | POST | `/v1/dashboard/news_playlists` | auth | Create a new playlist for the authenticated user |
//! | `dashboard_update` | PUT | `/v1/dashboard/news_playlists/{id}` | auth | Update a playlist (owner or admin only) |
//! | `dashboard_destroy` | DELETE | `/v1/dashboard/news_playlists/{id}` | auth | Delete a playlist (owner or admin only) |
//! | `dashboard_add_news` | POST | `/v1/dashboard/news_playlists/{id}/add_news` | auth | Add a news item to a playlist (owner or admin only) |
//! | `dashboard_reorder` | POST | `/v1/dashboard/news_playlists/{id}/reorder` | auth | Reorder news items in a playlist (owner or admin only) |
//! | `dashboard_remove_news` | POST | `/v1/dashboard/news_playlists/{playlist_id}/remove_news/{news_id}` | auth | Remove a news item from a playlist (owner or admin only) |

use actix_web::{web, HttpResponse};
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
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
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

/// List all playlists.
///
/// Requires admin role. Optionally filter by user_id query parameter.
///
/// # Response
///
/// `200 OK` — JSON array of `NewsPlaylistResponse`
/// `403 Forbidden` — insufficient role
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

/// Retrieve a single playlist by ID.
///
/// Requires admin role.
///
/// # Response
///
/// `200 OK` — `NewsPlaylistResponse`
/// `403 Forbidden` — insufficient role
/// `404 Not Found` — playlist does not exist
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

/// Delete a playlist and its associated users_news entries.
///
/// Requires admin role.
///
/// # Response
///
/// `200 OK` — confirmation message
/// `403 Forbidden` — insufficient role
/// `404 Not Found` — playlist does not exist
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

/// List playlists for the authenticated user.
///
/// Requires authentication.
///
/// # Response
///
/// `200 OK` — JSON array of `NewsPlaylistResponse`
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

/// Get a single playlist.
///
/// Requires authentication. Only the playlist owner or an admin can
/// access the playlist.
///
/// # Response
///
/// `200 OK` — `NewsPlaylistResponse`
/// `403 Forbidden` — not the owner and not an admin
/// `404 Not Found` — playlist does not exist
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

/// Create a new playlist for the authenticated user.
///
/// Requires authentication. Validates that name is non-empty.
///
/// # Response
///
/// `201 Created` — `NewsPlaylistResponse` for the new playlist
/// `422 Unprocessable` — empty name
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

/// Update a playlist.
///
/// Requires authentication. Only the playlist owner or an admin can
/// update. Supports partial updates — only provided fields are changed.
///
/// # Response
///
/// `200 OK` — `NewsPlaylistResponse` with updated values
/// `403 Forbidden` — not the owner and not an admin
/// `404 Not Found` — playlist does not exist
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

/// Delete a playlist and its associated users_news entries.
///
/// Requires authentication. Only the playlist owner or an admin can
/// delete.
///
/// # Response
///
/// `200 OK` — confirmation message
/// `403 Forbidden` — not the owner and not an admin
/// `404 Not Found` — playlist does not exist
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

/// Add a news item to a playlist.
///
/// Requires authentication. Only the playlist owner or an admin can
/// add. Automatically assigns the next available position.
///
/// # Response
///
/// `200 OK` — confirmation message
/// `403 Forbidden` — not the owner and not an admin
/// `404 Not Found` — playlist does not exist
/// `422 Unprocessable` — news_id is required
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

/// Reorder news items in a playlist.
///
/// Requires authentication. Only the playlist owner or an admin can
/// reorder. Accepts a news_ids array to determine new positions.
///
/// # Response
///
/// `200 OK` — confirmation message
/// `403 Forbidden` — not the owner and not an admin
/// `404 Not Found` — playlist does not exist
/// `422 Unprocessable` — news_ids array is required
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

/// Remove a news item from a playlist.
///
/// Requires authentication. Only the playlist owner or an admin can
/// remove.
///
/// # Response
///
/// `200 OK` — confirmation message
/// `403 Forbidden` — not the owner and not an admin
/// `404 Not Found` — playlist does not exist
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
    cfg.route("/admin/news_playlists", web::get().to(index_admin))
        .route("/admin/news_playlists/{id}", web::get().to(show_admin))
        .route("/admin/news_playlists/{id}", web::delete().to(destroy_admin))
        .route("/dashboard/news_playlists", web::get().to(dashboard_index))
        .route("/dashboard/news_playlists/{id}", web::get().to(dashboard_show))
        .route("/dashboard/news_playlists", web::post().to(dashboard_create))
        .route("/dashboard/news_playlists/{id}", web::put().to(dashboard_update))
        .route("/dashboard/news_playlists/{id}", web::delete().to(dashboard_destroy))
        .route("/dashboard/news_playlists/{id}/add_news", web::post().to(dashboard_add_news))
        .route("/dashboard/news_playlists/{id}/reorder", web::post().to(dashboard_reorder))
        .route("/dashboard/news_playlists/{playlist_id}/remove_news/{news_id}", web::post().to(dashboard_remove_news));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{admin_token, not_found_id, seed_user_with_id, unique_suffix, user_token};
    use actix_web::test;

    async fn seed_playlist(state: &AppState, suffix: &str) -> (i64, String) {
        seed_user_with_id(&state.db, 1, "admin@test.com", "admin").await;
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

    #[tokio::test(flavor = "current_thread")]
    async fn admin_playlists_index() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/admin/news_playlists")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn admin_playlist_show_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let s = unique_suffix();
        let (playlist_id, name) = seed_playlist(&state, &s).await;

        let req = test::TestRequest::get()
            .uri(&format!("/admin/news_playlists/{}", playlist_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["name"], name);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn admin_playlist_show_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "news_playlists").await;

        let req = test::TestRequest::get()
            .uri(&format!("/admin/news_playlists/{}", not_found))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn admin_playlist_destroy() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let s = unique_suffix();
        let (playlist_id, name) = seed_playlist(&state, &s).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/admin/news_playlists/{}", playlist_id))
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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_playlists_index() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let s = unique_suffix();
        let (_playlist_id, name) = seed_playlist(&state, &s).await;

        let req = test::TestRequest::get()
            .uri("/dashboard/news_playlists")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        let found = body.as_array().unwrap().iter().any(|v| v["name"] == name);
        assert!(found, "Created playlist should appear in user's list");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_create_playlist() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        seed_user_with_id(&state.db, 1, "admin@test.com", "admin").await;

        let s = unique_suffix();
        let req = test::TestRequest::post()
            .uri("/dashboard/news_playlists")
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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_create_empty_name() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/dashboard/news_playlists")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "name": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_update_playlist() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let s = unique_suffix();
        let (playlist_id, name) = seed_playlist(&state, &s).await;

        let req = test::TestRequest::put()
            .uri(&format!("/dashboard/news_playlists/{}", playlist_id))
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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_update_forbidden() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let s = unique_suffix();
        let (_playlist_id, name) = seed_playlist(&state, &s).await;

        let (playlist_id, _) = {
            let row: (i64,) = sqlx::query_as(r"SELECT id FROM news_playlists WHERE name = $1")
                .bind(&name)
                .fetch_one(&state.db)
                .await
                .expect("Failed to find playlist");
            (row.0, name.clone())
        };

        let req = test::TestRequest::put()
            .uri(&format!("/dashboard/news_playlists/{}", playlist_id))
            .insert_header(("Authorization", format!("Bearer {}", user_token(9999))))
            .set_json(serde_json::json!({
                "name": "Hacked"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_destroy_playlist() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let s = unique_suffix();
        let (playlist_id, name) = seed_playlist(&state, &s).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/dashboard/news_playlists/{}", playlist_id))
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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_destroy_forbidden() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let s = unique_suffix();
        let (playlist_id, name) = seed_playlist(&state, &s).await;

        let req = test::TestRequest::delete()
            .uri(&format!("/dashboard/news_playlists/{}", playlist_id))
            .insert_header(("Authorization", format!("Bearer {}", user_token(9999))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }
}
