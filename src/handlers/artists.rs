use actix_web::{web, HttpResponse};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_artist_or_above;
use crate::error::AppError;
use crate::models::artist::{Artist, ArtistResponse, CreateArtistRequest, UpdateArtistRequest};
use crate::models::artist_link::{ArtistLink, CreateArtistLinkRequest};
use crate::services::storage_service;

#[derive(Debug, FromRow)]
struct ArtistRow {
    id: i64,
    name: Option<String>,
    genre: Option<String>,
    bio: Option<String>,
    user_id: Option<i64>,
    prospect: Option<bool>,
    spotify_id: Option<String>,
    sub_heading: Option<String>,
    intro: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<ArtistRow> for Artist {
    fn from(row: ArtistRow) -> Self {
        Artist {
            id: row.id,
            name: row.name,
            genre: row.genre,
            bio: row.bio,
            user_id: row.user_id,
            prospect: row.prospect,
            spotify_id: row.spotify_id,
            sub_heading: row.sub_heading,
            intro: row.intro,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

  pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let rows = sqlx::query_as::<_, ArtistRow>(
        r#"SELECT id, name, genre, bio, user_id, prospect,
           "spotifyId" AS spotify_id, "subHeading" AS sub_heading, intro,
           created_at, updated_at FROM artists ORDER BY id"#
    )
    .fetch_all(&state.db)
    .await?;

    let artists: Vec<Artist> = rows.into_iter().map(|r| r.into()).collect();
    let mut responses: Vec<ArtistResponse> = Vec::new();
    for artist in &artists {
        let (image_urls, thumbnail_urls) = storage_service::get_image_urls(
            &state.s3,
            &state.db,
            "Artist",
            artist.id,
        ).await.unwrap_or_else(|_| (Vec::new(), Vec::new()));
        responses.push(artist.to_response(image_urls, thumbnail_urls));
    }
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    match sqlx::query_as::<_, ArtistRow>(
        r#"SELECT id, name, genre, bio, user_id, prospect,
           "spotifyId" AS spotify_id, "subHeading" AS sub_heading, intro,
           created_at, updated_at FROM artists WHERE id = $1"#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let artist: Artist = row.into();
            let (image_urls, thumbnail_urls) = storage_service::get_image_urls(
                &state.s3,
                &state.db,
                "Artist",
                artist.id,
            ).await.unwrap_or_else(|_| (Vec::new(), Vec::new()));
            Ok(HttpResponse::Ok().json(artist.to_response(image_urls, thumbnail_urls)))
        }
        None => Err(AppError::NotFound(format!("Artist #{}", id))),
    }
}

pub async fn create(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<CreateArtistRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    if let Some(ref intro) = body.intro
        && !Artist::validate_intro(intro) {
            return Err(AppError::Validation("Intro must be 300 characters or less".to_string()));
        }
    if let Some(ref bio) = body.bio
        && !Artist::validate_bio(bio) {
            return Err(AppError::Validation("Bio must be 3000 characters or less".to_string()));
        }

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, ArtistRow>(
        r#"INSERT INTO artists (name, genre, bio, prospect, "spotifyId", "subHeading", intro, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
           RETURNING id, name, genre, bio, user_id, prospect, "spotifyId" AS spotify_id, "subHeading" AS sub_heading, intro, created_at, updated_at"#
    )
    .bind(&body.name)
    .bind(body.genre.as_deref())
    .bind(body.bio.as_deref())
    .bind(body.prospect)
    .bind(body.spotify_id.as_deref())
    .bind(body.sub_heading.as_deref())
    .bind(body.intro.as_deref())
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let artist: Artist = row.into();
    Ok(HttpResponse::Created().json(artist.to_response(Vec::new(), Vec::new())))
}

pub async fn update(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateArtistRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();

    if let Some(ref bio) = body.bio
        && !Artist::validate_bio(bio) {
            return Err(AppError::Validation("Bio must be 3000 characters or less".to_string()));
        }
    if let Some(ref intro) = body.intro
        && !Artist::validate_intro(intro) {
            return Err(AppError::Validation("Intro must be 300 characters or less".to_string()));
        }

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, ArtistRow>(
        r#"UPDATE artists SET
            name = COALESCE($1, name),
            genre = COALESCE($2, genre),
            bio = COALESCE($3, bio),
            "spotifyId" = COALESCE($4, "spotifyId"),
            "subHeading" = COALESCE($5, "subHeading"),
            intro = COALESCE($6, intro),
            updated_at = $7
        WHERE id = $8
        RETURNING id, name, genre, bio, user_id, prospect, "spotifyId" AS spotify_id, "subHeading" AS sub_heading, intro, created_at, updated_at"#
    )
    .bind(body.name.as_deref())
    .bind(body.genre.as_deref())
    .bind(body.bio.as_deref())
    .bind(body.spotify_id.as_deref())
    .bind(body.sub_heading.as_deref())
    .bind(body.intro.as_deref())
    .bind(now)
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::NotFound(format!("Artist #{}", id)))?;

    let artist: Artist = row.into();
    Ok(HttpResponse::Ok().json(artist.to_response(Vec::new(), Vec::new())))
}

pub async fn add_artist_links(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<CreateArtistLinkRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    if !ArtistLink::validate_url(&body.url) {
        return Err(AppError::Validation("Invalid URL".to_string()));
    }

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, ArtistLink>(
        r"INSERT INTO artist_links (artist_id, link_type, url, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, artist_id, link_type, url, created_at AT TIME ZONE 'UTC' AS created_at, updated_at AT TIME ZONE 'UTC' AS updated_at"
    )
    .bind(body.artist_id)
    .bind(body.link_type)
    .bind(&body.url)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    Ok(HttpResponse::Created().json(row.to_response()))
}

pub async fn delete_artist_links(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let link_id = body.get("id").and_then(|v| v.as_i64()).ok_or_else(|| {
        AppError::Validation("Link id is required".to_string())
    })?;

    let result = sqlx::query(
        r"DELETE FROM artist_links WHERE id = $1"
    )
    .bind(link_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Artist link #{}", link_id)));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Artist link #{} deleted", link_id)
    })))
}

pub async fn available_link_types() -> Result<HttpResponse, AppError> {
    let types = ArtistLink::available_link_types();
    Ok(HttpResponse::Ok().json(types))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/artists", web::get().to(index))
            .route("/artist/{id}", web::get().to(show))
            .route("/artist", web::post().to(create))
            .route("/artist/{id}", web::put().to(update))
            .route("/artist/add_artist_links", web::post().to(add_artist_links))
            .route("/artist/delete_artist_links", web::post().to(delete_artist_links))
            .route("/available_link_types", web::get().to(available_link_types)),
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
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO users (email, password_digest, role, created_at, updated_at)
               VALUES ($1, $2, $3, NOW(), NOW())
               RETURNING id"
        )
        .bind(format!("artist_test_{}@test.com", ts))
        .bind("hashed_password_test".to_string())
        .bind(Some("artist".to_string()))
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed user")
    }

    async fn seed_artist(user_id: i64) -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO artists (name, genre, bio, user_id, prospect, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
               RETURNING id"
        )
        .bind(format!("Test Artist {}", ts))
        .bind(Some("Electronic".to_string()))
        .bind(Some("A test artist bio".to_string()))
        .bind(Some(user_id))
        .bind(Some(false))
        .fetch_one(&get_state().await.db)
        .await
        .expect("Failed to seed artist")
    }

    async fn cleanup_user(user_id: i64) {
        let _ = sqlx::query(r"DELETE FROM artists WHERE user_id = $1")
            .bind(user_id)
            .execute(&get_state().await.db)
            .await;
        let _ = sqlx::query(r"DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&get_state().await.db)
            .await;
    }

    async fn cleanup_artist(artist_id: i64) {
        let _ = sqlx::query(r"DELETE FROM artist_links WHERE artist_id = $1")
            .bind(artist_id)
            .execute(&get_state().await.db)
            .await;
        let _ = sqlx::query(r"DELETE FROM artists WHERE id = $1")
            .bind(artist_id)
            .execute(&get_state().await.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artists_index_public() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get().uri("/v1/artists").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_show_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;

        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/v1/artist/{}", artist_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], artist_id);

        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_show_not_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM artists"
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
            .uri(&format!("/v1/artist/{}", max_id + 9999))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_create_admin() {
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

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let name = format!("New Artist {}", ts);

        let req = test::TestRequest::post()
            .uri("/v1/artist")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "name": name,
                "genre": "Jazz"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["genre"], "Jazz");

        let _ = sqlx::query(r"DELETE FROM artists WHERE name = $1")
            .bind(&name)
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_create_forbidden_non_admin() {
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
            .uri("/v1/artist")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "name": "Should Fail"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_update_success() {
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

        let req = test::TestRequest::put()
            .uri(&format!("/v1/artist/{}", artist_id))
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .set_json(serde_json::json!({
                "name": "Updated Artist Name"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["name"], "Updated Artist Name");

        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_update_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM artists"
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

        let req = test::TestRequest::put()
            .uri(&format!("/v1/artist/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .set_json(serde_json::json!({
                "name": "Updated"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_artist_link_success() {
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

        let req = test::TestRequest::post()
            .uri("/v1/artist/add_artist_links")
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .set_json(serde_json::json!({
                "artist_id": artist_id,
                "link_type": 1,
                "url": "https://spotify.com/test"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["artist_id"], artist_id);
        assert_eq!(body["link_type"], 1);

        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_artist_link_invalid_url() {
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
            .uri("/v1/artist/add_artist_links")
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .set_json(serde_json::json!({
                "artist_id": 1,
                "link_type": 1,
                "url": "not-a-url"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_artist_link_success() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);

        let user_id = seed_user().await;
        let artist_id = seed_artist(user_id).await;

        let now = chrono::Utc::now().naive_utc();
        let link_id: i64 = sqlx::query_scalar(
            r"INSERT INTO artist_links (artist_id, link_type, url, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id"
        )
        .bind(artist_id)
        .bind(1)
        .bind("https://spotify.com/test")
        .bind(&now)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed artist link");

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/artist/delete_artist_links")
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .set_json(serde_json::json!({
                "id": link_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let remaining: i64 = sqlx::query_scalar(
            r"SELECT COUNT(*) FROM artist_links WHERE id = $1"
        )
        .bind(link_id)
        .fetch_one(&state.db)
        .await
        .expect("Failed to check link");
        assert_eq!(remaining, 0);

        cleanup_artist(artist_id).await;
        cleanup_user(user_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_artist_link_not_found() {
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
            .uri("/v1/artist/delete_artist_links")
            .insert_header(("Authorization", format!("Bearer {}", artist_token())))
            .set_json(serde_json::json!({
                "id": 99999999
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn available_link_types_public() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get().uri("/v1/available_link_types").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body.as_array().unwrap().len(), 11);
    }
}
