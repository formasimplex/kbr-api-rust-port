//! Song handlers
//!
//! Provides CRUD endpoints for song/track management. Songs are public to read;
//! creation requires admin role.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/songs` | public | List all songs |
//! | `show` | GET | `/v1/song/{id}` | public | Retrieve a single song by ID |
//! | `create` | POST | `/v1/songs` | admin | Create a new song |

use actix_web::{web, HttpResponse};

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::data::songs as data;
use crate::error::AppError;
use crate::models::song::{CreateSongRequest, SongResponse};

/// List all songs.
///
/// Returns all songs ordered by ID. No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `SongResponse`
pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let songs = data::list(&state.db).await?;
    let responses: Vec<SongResponse> = songs.iter().map(|s| s.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Retrieve a single song by ID.
///
/// No authentication required.
///
/// # Response
///
/// `200 OK` — `SongResponse`
/// `404 Not Found` — song does not exist
pub async fn show(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    match data::by_id(&state.db, id).await? {
        Some(song) => Ok(HttpResponse::Ok().json(song.to_response())),
        None => Err(AppError::NotFound(format!("Song #{}", id))),
    }
}

/// Create a new song.
///
/// Requires admin role. Associates the song with an album and artist via IDs.
///
/// # Response
///
/// `201 Created` — `SongResponse` for the new song
/// `403 Forbidden` — non-admin user
pub async fn create(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<CreateSongRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not authorized".to_string()));
    }

    let now = chrono::Utc::now().naive_utc();

    let song = data::create(&state.db, &body.name, &body.duration, body.album_id, body.artist_id, now).await?;
    Ok(HttpResponse::Created().json(song.to_response()))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/songs", web::get().to(index))
        .route("/song/{id}", web::get().to(show))
        .route("/songs", web::post().to(create));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token_with_role;
    use crate::test_utils::{not_found_id, TEST_SECRET};
    use actix_web::test;

    async fn seed_album_and_artist(pool: &sqlx::PgPool) -> (i64, i64) {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let test_artist_name = format!("Test Artist Songs {}", suffix);
        let test_album_name = format!("Test Album Songs {}", suffix);

        let aid: i64 = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO artists (name, created_at, updated_at)
               VALUES ($1, NOW(), NOW())
               RETURNING id"
        )
        .bind(&test_artist_name)
        .fetch_one(pool)
        .await
        .expect("Failed to seed artist");

        let alb_id: i64 = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO albums (name, release_date, created_at, updated_at)
               VALUES ($1, '2024-01-01', NOW(), NOW())
               RETURNING id"
        )
        .bind(&test_album_name)
        .fetch_one(pool)
        .await
        .expect("Failed to seed album");

        (alb_id, aid)
    }

    #[tokio::test(flavor = "current_thread")]
    async fn songs_index_returns_ok() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get().uri("/songs").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn song_show_found() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let (album_id, artist_id) = seed_album_and_artist(&state.db).await;

        let seed = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO songs (name, duration, album_id, artist_id, created_at, updated_at)
               VALUES ('Test Song SQLx', '3:45', $1, $2, NOW(), NOW())
               RETURNING id"
        )
        .bind(album_id)
        .bind(artist_id)
        .fetch_one(&state.db)
        .await;

        if let Ok(id) = seed {
            let req = test::TestRequest::get()
                .uri(&format!("/song/{}", id))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);

            let body: serde_json::Value = test::read_body_json(resp).await;
            assert_eq!(body["name"], "Test Song SQLx");
        }

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn song_show_not_found() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "songs").await;

        let req = test::TestRequest::get()
            .uri(&format!("/song/{}", not_found))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn song_create_admin() {
        crate::test_utils::set_test_env_jwt();

        let token = encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let (album_id, artist_id) = seed_album_and_artist(&state.db).await;

        let req = test::TestRequest::post()
            .uri("/songs")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "name": "Rust Test Song",
                "duration": "4:20",
                "album_id": album_id,
                "artist_id": artist_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(status, 201, "Expected 201, got {}: {:?}", status, body);
        assert_eq!(body["name"], "Rust Test Song");
        assert_eq!(body["duration"], "4:20");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn song_create_forbidden_non_admin() {
        crate::test_utils::set_test_env_jwt();

        let token = encode_token_with_role(2, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/songs")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "name": "Should Fail",
                "album_id": 1,
                "artist_id": 1
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }
}
