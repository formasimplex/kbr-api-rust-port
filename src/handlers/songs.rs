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
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::song::{CreateSongRequest, Song, SongResponse};

#[derive(Debug, FromRow)]
struct SongRow {
    id: i64,
    name: Option<String>,
    duration: Option<String>,
    album_id: i64,
    artist_id: i64,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<SongRow> for Song {
    fn from(row: SongRow) -> Self {
        Song {
            id: row.id,
            name: row.name,
            duration: row.duration,
            album_id: row.album_id,
            artist_id: row.artist_id,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

/// List all songs.
///
/// Returns all songs ordered by ID. No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `SongResponse`
pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let rows = sqlx::query_as::<_, SongRow>(
        r"SELECT id, name, duration, album_id, artist_id, created_at, updated_at FROM songs ORDER BY id"
    )
    .fetch_all(&state.db)
    .await?;

    let songs: Vec<Song> = rows.into_iter().map(|r| r.into()).collect();
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

    match sqlx::query_as::<_, SongRow>(
        r"SELECT id, name, duration, album_id, artist_id, created_at, updated_at FROM songs WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let song: Song = row.into();
            Ok(HttpResponse::Ok().json(song.to_response()))
        }
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

    let row = sqlx::query_as::<_, SongRow>(
        r"INSERT INTO songs (name, duration, album_id, artist_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, name, duration, album_id, artist_id, created_at, updated_at"
    )
    .bind(&body.name)
    .bind(&body.duration)
    .bind(body.album_id)
    .bind(body.artist_id)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let song: Song = row.into();
    Ok(HttpResponse::Created().json(song.to_response()))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/songs", web::get().to(index))
            .route("/song/{id}", web::get().to(show))
            .route("/songs", web::post().to(create)),
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
        crate::test_utils::build_test_state(pool).await
    }

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
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get().uri("/v1/songs").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn song_show_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let (album_id, artist_id) = seed_album_and_artist(&state.db).await;

        let seed = sqlx::query_as::<_, SongRow>(
            r"INSERT INTO songs (name, duration, album_id, artist_id, created_at, updated_at)
               VALUES ('Test Song SQLx', '3:45', $1, $2, NOW(), NOW())
               RETURNING id, name, duration, album_id, artist_id, created_at, updated_at"
        )
        .bind(album_id)
        .bind(artist_id)
        .fetch_one(&state.db)
        .await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        if let Ok(row) = seed {
            let id = row.id;
            let req = test::TestRequest::get()
                .uri(&format!("/v1/song/{}", id))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);

            let body: serde_json::Value = test::read_body_json(resp).await;
            assert_eq!(body["name"], "Test Song SQLx");

            let _ = sqlx::query(&format!(r"DELETE FROM songs WHERE id = {}", id))
                .execute(&state.db)
                .await;
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn song_show_not_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM songs"
        )
        .fetch_one(&state.db)
        .await
        .expect("Failed to get max id");

        let req = test::TestRequest::get()
            .uri(&format!("/v1/song/{}", max_id + 9999))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn song_create_admin() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let token = encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap();
        let state = web::Data::new(get_state().await);
        let (album_id, artist_id) = seed_album_and_artist(&state.db).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/songs")
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

        let _ = sqlx::query(
            r"DELETE FROM songs WHERE name = 'Rust Test Song'"
        )
        .execute(&state.db)
        .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn song_create_forbidden_non_admin() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let token = encode_token_with_role(2, TEST_SECRET, 3, Some("user".to_string())).unwrap();
        let state = web::Data::new(get_state().await);

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/songs")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "name": "Should Fail",
                "album_id": 1,
                "artist_id": 1
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }
}
