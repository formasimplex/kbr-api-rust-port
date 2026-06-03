//! Album handlers
//!
//! Provides CRUD endpoints for album management. Albums are public to read;
//! creation requires admin role.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/albums` | public | List all albums |
//! | `show` | GET | `/v1/album/{id}` | public | Retrieve a single album by ID |
//! | `create` | POST | `/v1/albums` | admin | Create a new album |

use actix_web::{HttpResponse, web};
use chrono::NaiveDate;
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::album::{Album, AlbumResponse, CreateAlbumRequest};

#[derive(Debug, FromRow)]
struct AlbumRow {
    id: i64,
    name: Option<String>,
    release_date: NaiveDate,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<AlbumRow> for Album {
    fn from(row: AlbumRow) -> Self {
        Album {
            id: row.id,
            name: row.name,
            release_date: Some(row.release_date.to_string()),
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

/// List all albums.
///
/// Returns all albums ordered by ID. No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `AlbumResponse`
pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let rows = sqlx::query_as::<_, AlbumRow>(
        r"SELECT id, name, release_date, created_at, updated_at FROM albums ORDER BY id",
    )
    .fetch_all(&state.db)
    .await?;

    let albums: Vec<Album> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<AlbumResponse> = albums.iter().map(|a| a.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Retrieve a single album by ID.
///
/// No authentication required.
///
/// # Response
///
/// `200 OK` — `AlbumResponse`
/// `404 Not Found` — album does not exist
pub async fn show(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    match sqlx::query_as::<_, AlbumRow>(
        r"SELECT id, name, release_date, created_at, updated_at FROM albums WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let album: Album = row.into();
            Ok(HttpResponse::Ok().json(album.to_response()))
        }
        None => Err(AppError::NotFound(format!("Album #{}", id))),
    }
}

/// Create a new album.
///
/// Validates release_date format (YYYY-MM-DD). Requires admin role.
///
/// # Response
///
/// `201 Created` — `AlbumResponse` for the new album
/// `400 Bad Request` — invalid release_date format
/// `403 Forbidden` — non-admin user
pub async fn create(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<CreateAlbumRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not authorized".to_string()));
    }

    let release_date = body
        .release_date
        .as_ref()
        .map(|s| {
            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                .map_err(|e| AppError::BadRequest(format!("Invalid release_date format: {}", e)))
        })
        .transpose()?;

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, AlbumRow>(
        r"INSERT INTO albums (name, release_date, created_at, updated_at)
           VALUES ($1, $2, $3, $4)
           RETURNING id, name, release_date, created_at, updated_at",
    )
    .bind(&body.name)
    .bind(release_date)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let album: Album = row.into();
    Ok(HttpResponse::Created().json(album.to_response()))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/albums", web::get().to(index))
        .route("/album/{id}", web::get().to(show))
        .route("/albums", web::post().to(create));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token_with_role;
    use crate::test_utils::{get_test_state, not_found_id, TEST_SECRET};
    use actix_web::{App, test};

   #[tokio::test(flavor = "current_thread")]
    async fn albums_index_returns_ok() {
        crate::test_utils::set_test_env();
        let (_state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get().uri("/albums").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn album_show_found() {
        crate::test_utils::set_test_env();
        let state = web::Data::new(get_test_state().await);

        let seed = sqlx::query_as::<_, AlbumRow>(
            r"INSERT INTO albums (name, release_date, created_at, updated_at)
              VALUES ('Test Album SQLx', '2024-01-15', NOW(), NOW())
              RETURNING id, name, release_date, created_at, updated_at",
        )
        .fetch_one(&state.db)
        .await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_test_state().await))
                .configure(config_routes),
        )
        .await;

        if let Ok(row) = seed {
            let id = row.id;
            let req = test::TestRequest::get()
                .uri(&format!("/album/{}", id))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);

            let body: serde_json::Value = test::read_body_json(resp).await;
            assert_eq!(body["name"], "Test Album SQLx");

            let _ = sqlx::query(&format!("DELETE FROM albums WHERE id = {}", id))
                .execute(&state.db)
                .await;
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn album_show_not_found() {
        crate::test_utils::set_test_env();
        let (state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "albums").await;

        let req = test::TestRequest::get()
            .uri(&format!("/album/{}", not_found))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn album_create_admin() {
        crate::test_utils::set_test_env_jwt();

        let token =
            encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap();
        let (state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/albums")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "name": "Rust Test Album",
                "release_date": "2025-06-01"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(status, 201, "Expected 201, got {}: {:?}", status, body);
        assert_eq!(body["name"], "Rust Test Album");
        assert_eq!(body["release_date"], "2025-06-01");

        let _ = sqlx::query(r"DELETE FROM albums WHERE name = 'Rust Test Album'")
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn album_create_forbidden_non_admin() {
        crate::test_utils::set_test_env_jwt();

        let token = encode_token_with_role(2, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();
        let (_state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/albums")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "name": "Should Fail"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }
}
