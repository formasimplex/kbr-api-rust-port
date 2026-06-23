//! Artist handlers
//!
//! Provides CRUD endpoints for artist management, including artist links
//! (social media, streaming platforms). Reading is public; creation requires
//! admin; updates and link management require artist+ role.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/artists` | public | List all artists with images |
//! | `show` | GET | `/v1/artist/{id}` | public | Retrieve a single artist with images |
//! | `create` | POST | `/v1/artist` | admin | Create a new artist |
//! | `update` | PUT | `/v1/artist/{id}` | artist+ | Update artist details |
//! | `add_artist_links` | POST | `/v1/artist/add_artist_links` | artist+ | Add a social/streaming link |
//! | `delete_artist_links` | POST | `/v1/artist/delete_artist_links` | artist+ | Remove an artist link |
//! | `available_link_types` | GET | `/v1/available_link_types` | public | List valid link type options |

use actix_multipart::Multipart;
use actix_web::{HttpResponse, web};
use uuid::Uuid;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::permissions::is_artist_or_above;
use crate::data::artists as data;
use crate::error::AppError;
use crate::models::artist::{Artist, ArtistResponse, CreateArtistRequest, UpdateArtistRequest};
use crate::models::artist_link::{ArtistLink, ArtistLinkResponse, CreateArtistLinkRequest};
use crate::models::sign_up_trigger::SignUpTrigger;
use crate::models::user::User;
use crate::services::storage;
use crate::services::user_service::UserService;

/// Request body for artist sign-up via token.
#[derive(serde::Deserialize)]
pub(crate) struct ArtistSignUpRequest {
    token: String,
    name: String,
    password: String,
    password_confirmation: String,
    username: Option<String>,
}



/// Fetch artist links for a set of artist IDs in a single query.
///
/// Returns a HashMap mapping artist_id -> Vec<ArtistLinkResponse>.
async fn fetch_artist_links_batch(
    pool: &sqlx::PgPool,
    artist_ids: &[i64],
) -> std::collections::HashMap<i64, Vec<ArtistLinkResponse>> {
    if artist_ids.is_empty() {
        return std::collections::HashMap::new();
    }
    let rows = sqlx::query_as::<_, ArtistLink>(
        r"SELECT id, artist_id, link_type, url, created_at AT TIME ZONE 'UTC' AS created_at, updated_at AT TIME ZONE 'UTC' AS updated_at
           FROM artist_links WHERE artist_id = ANY($1) ORDER BY artist_id, id",
    )
    .bind(artist_ids)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut map = std::collections::HashMap::new();
    for row in rows {
        map.entry(row.artist_id)
            .or_insert_with(Vec::new)
            .push(row.to_response());
    }
    map
}

/// List all artists.
///
/// Returns all artists ordered by ID, including associated image and thumbnail
/// URLs from S3 storage. No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `ArtistResponse`
pub async fn index(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let artists = data::list(&state.db).await?;
    let artist_ids: Vec<i64> = artists.iter().map(|a| a.id).collect();
    let links_map = fetch_artist_links_batch(&state.db, &artist_ids).await;
    let mut responses: Vec<ArtistResponse> = Vec::new();
    for artist in &artists {
        let (image_urls, thumbnail_urls) =
            storage::get_image_urls(&state.s3, &state.db, "Artist", artist.id)
                .await
                .unwrap_or_else(|_| (Vec::new(), Vec::new()));
        let links = links_map.get(&artist.id).cloned().unwrap_or_default();
        responses.push(artist.to_response(image_urls, thumbnail_urls, links));
    }
    Ok(HttpResponse::Ok().json(responses))
}

/// Retrieve a single artist by ID.
///
/// Includes associated image and thumbnail URLs from S3 storage.
/// No authentication required.
///
/// # Response
///
/// `200 OK` — `ArtistResponse`
/// `404 Not Found` — artist does not exist
pub async fn show(
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    match data::by_id(&state.db, id).await? {
        Some(artist) => {
            let (image_urls, thumbnail_urls) =
                storage::get_image_urls(&state.s3, &state.db, "Artist", artist.id)
                    .await
                    .unwrap_or_else(|_| (Vec::new(), Vec::new()));
            let links_map = fetch_artist_links_batch(&state.db, &[artist.id]).await;
            let links = links_map.get(&artist.id).cloned().unwrap_or_default();
            Ok(HttpResponse::Ok().json(artist.to_response(image_urls, thumbnail_urls, links)))
        }
        None => Err(AppError::NotFound(format!("Artist #{}", id))),
    }
}

/// Create a new artist.
///
/// Validates intro (max 300 chars) and bio (max 3000 chars). Requires admin role.
///
/// # Response
///
/// `201 Created` — `ArtistResponse` for the new artist
/// `403 Forbidden` — non-admin user
/// `422 Unprocessable Entity` — validation error (intro/bio too long)
pub async fn create(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<CreateArtistRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    if let Some(ref intro) = body.intro
        && !Artist::validate_intro(intro)
    {
        return Err(AppError::Validation(
            "Intro has to be 300 characters or less".to_string(),
        ));
    }
    if let Some(ref bio) = body.bio
        && !Artist::validate_bio(bio)
    {
        return Err(AppError::Validation(
            "Bio has to be 3000 characters or less".to_string(),
        ));
    }

    let now = chrono::Utc::now().naive_utc();

    let artist = data::create(&state.db, &body.name, &body.genre, &body.bio, body.prospect, &body.spotify_id, &body.sub_heading, &body.intro, now).await?;
    Ok(HttpResponse::Created().json(artist.to_response(Vec::new(), Vec::new(), Vec::new())))
}

/// Create a new artist account via sign-up trigger token.
///
/// Public endpoint. Uses a database transaction with `SELECT ... FOR UPDATE`
/// to lock the sign-up trigger row, preventing concurrent token reuse.
/// Checks for an existing user with the same email before creating a new one.
/// Creates a user with "artist" role using the provided password and optional
/// username, creates the artist record with the given name and `prospect = true`,
/// and consumes the trigger. Enqueues a prospect welcome email job after
/// successful commit.
///
/// # Response
///
/// `201 Created` — `ArtistResponse` for the new artist
/// `409 Conflict` — email already has an artist account
/// `422 Unprocessable Entity` — invalid token, password mismatch, or weak password
pub(crate) async fn sign_up(
    state: web::Data<AppState>,
    body: web::Json<ArtistSignUpRequest>,
) -> Result<HttpResponse, AppError> {
    let mut tx = state.db.begin().await?;

    let trigger = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        r"SELECT email, expires_at FROM sign_up_triggers WHERE token = $1 FOR UPDATE",
    )
    .bind(&body.token)
    .fetch_optional(&mut *tx)
    .await?;

    match trigger {
        Some((Some(trigger_email), trigger_expires)) => {
            if let Some(expires_str) = trigger_expires
                && let Some(expired_time) = SignUpTrigger::parse_expires_at(&expires_str)
                    && expired_time < chrono::Utc::now() {
                        return Err(AppError::Validation(
                            "Sign-up token has expired".to_string(),
                        ));
                    }

            if !User::validate_email(&trigger_email) {
                return Err(AppError::Validation("Invalid sign-up token".to_string()));
            }

            if body.password != body.password_confirmation {
                return Err(AppError::Validation(
                    "Password and confirmation do not match".to_string(),
                ));
            }

            if !User::validate_password(&body.password) {
                return Err(AppError::Validation(
                    "Password must be at least 8 characters with uppercase, lowercase, and a digit"
                        .to_string(),
                ));
            }

            let existing: bool = sqlx::query_scalar(
                r"SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 AND role = 'artist')",
            )
            .bind(&trigger_email)
            .fetch_one(&mut *tx)
            .await?;

            if existing {
                return Err(AppError::Conflict(
                    "Email already linked to artist account".to_string(),
                ));
            }

            let now = chrono::Utc::now().naive_utc();
            let password_digest =
                UserService::hash_password_for_create(&crate::models::user::CreateUserRequest {
                    email: trigger_email.clone(),
                    password: body.password.clone(),
                    username: body.username.clone(),
                    token: None,
                })?;

            let user_id: i64 = sqlx::query_scalar(
                r"INSERT INTO users (email, password_digest, role, username, created_at, updated_at)
                   VALUES ($1, $2, 'artist', $3, $4, $4) RETURNING id",
            )
            .bind(&trigger_email)
            .bind(&password_digest)
            .bind(body.username.as_deref())
            .bind(now)
            .fetch_one(&mut *tx)
            .await?;

            let artist_row = data::create_with_user(&mut tx, &body.name, user_id, now).await?;

            let now_str = chrono::Utc::now().to_rfc3339();
            sqlx::query(
                r"UPDATE sign_up_triggers SET expires_at = $1, updated_at = $2 WHERE email = $3",
            )
            .bind(&now_str)
            .bind(now)
            .bind(&trigger_email)
            .execute(&mut *tx)
            .await?;

            tx.commit().await?;

            let job_id = Uuid::new_v4();
            if let Err(e) = state
                .job_handle
                .send(crate::jobs::Job::SendProspectWelcomeEmail { job_id, user_id })
                .await
            {
                tracing::warn!(job_id = %job_id, user_id, error = %e, "Failed to enqueue prospect welcome email job");
            }

            let artist: Artist = artist_row.into();
            Ok(HttpResponse::Created().json(artist.to_response(Vec::new(), Vec::new(), Vec::new())))
        }
        _ => Err(AppError::Validation("Invalid sign-up token".to_string())),
    }
}

/// Update artist details.
///
/// Accepts multipart/form-data with fields prefixed by `artist[` (e.g.,
/// `artist[name]`, `artist[bio]`, `artist[subHeading]`, `artist[intro]`).
/// Also accepts image files via `artist[images][]` which are uploaded to S3
/// and attached to the artist record.
///
/// Validates intro (max 300 chars) and bio (max 3000 chars). Requires artist+ role.
///
/// # Response
///
/// `200 OK` — updated `ArtistResponse` with image URLs
/// `403 Forbidden` — role below artist
/// `404 Not Found` — artist does not exist
/// `422 Unprocessable Entity` — validation error (intro/bio too long)
pub async fn update(
    state: web::Data<AppState>,
    user: CurrentUser,
    path: web::Path<i64>,
    payload: Multipart,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();

    let parsed = UpdateArtistRequest::from_multipart(payload).await?;
    let UpdateArtistRequest {
        name,
        genre,
        bio,
        spotify_id,
        sub_heading,
        intro,
    } = parsed.request;
    let image_files = parsed.image_files;

    // Validate bio
    if let Some(ref bio_val) = bio
        && !Artist::validate_bio(bio_val)
    {
        return Err(AppError::Validation(
            "Bio must be 3000 characters or less".to_string(),
        ));
    }

    // Validate intro
    if let Some(ref intro_val) = intro
        && !Artist::validate_intro(intro_val)
    {
        return Err(AppError::Validation(
            "Intro must be 300 characters or less".to_string(),
        ));
    }

    // Upload image files to S3 and attach to artist
    for (file_data, filename, content_type) in image_files {
        storage::upload_and_attach(
            &state.s3,
            &state.db,
            "Artist",
            id,
            "images",
            &file_data,
            &filename,
            &content_type,
        )
        .await?;
    }

    let now = chrono::Utc::now().naive_utc();

    let artist = data::update(&state.db, id, &name, &genre, &bio, &spotify_id, &sub_heading, &intro, now).await?
        .ok_or_else(|| AppError::NotFound(format!("Artist #{}", id)))?;

    let (image_urls, thumbnail_urls) =
        storage::get_image_urls(&state.s3, &state.db, "Artist", artist.id)
            .await
            .unwrap_or_else(|_| (Vec::new(), Vec::new()));

    let links_map = fetch_artist_links_batch(&state.db, &[artist.id]).await;
    let links = links_map.get(&artist.id).cloned().unwrap_or_default();

    Ok(HttpResponse::Ok().json(artist.to_response(image_urls, thumbnail_urls, links)))
}

/// Add a social/streaming link for an artist.
///
/// Validates the URL format. Requires artist+ role.
///
/// # Response
///
/// `201 Created` — `ArtistLinkResponse` for the new link
/// `403 Forbidden` — role below artist
/// `422 Unprocessable Entity` — invalid URL format
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

/// Remove an artist link by ID.
///
/// Accepts a JSON body with an `id` field. Requires artist+ role.
///
/// # Response
///
/// `200 OK` — confirmation message
/// `403 Forbidden` — role below artist
/// `404 Not Found` — link does not exist
/// `422 Unprocessable Entity` — missing link id
pub async fn delete_artist_links(
    state: web::Data<AppState>,
    user: CurrentUser,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let link_id = body
        .get("id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| AppError::Validation("Link id is required".to_string()))?;

    let result = sqlx::query(r"DELETE FROM artist_links WHERE id = $1")
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

/// List available artist link types.
///
/// Returns the set of valid link type identifiers (e.g., Spotify, Instagram)
/// that can be used when creating artist links. No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of link type objects
pub async fn available_link_types() -> Result<HttpResponse, AppError> {
    let types = ArtistLink::available_link_types();
    Ok(HttpResponse::Ok().json(types))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/artists", web::get().to(index))
        .route("/artist/{id}", web::get().to(show))
        .route("/artist", web::post().to(create))
        .route("/artist/sign_up", web::post().to(sign_up))
        .route("/artist/{id}", web::put().to(update))
        .route("/artist/add_artist_links", web::post().to(add_artist_links))
        .route("/artist/delete_artist_links", web::post().to(delete_artist_links))
        .route("/available_link_types", web::get().to(available_link_types));
}

#[cfg(test)]
mod tests {
    use super::*;
 use crate::auth::jwt::encode_token_with_role;
     use crate::test_utils::{admin_token, artist_token, not_found_id, seed_artist, seed_test_user, TEST_SECRET};
    use actix_web::test;

    #[tokio::test(flavor = "current_thread")]
    async fn artists_index_public() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get().uri("/artists").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_show_found() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "artist_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let req = test::TestRequest::get()
            .uri(&format!("/artist/{}", artist_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["id"], artist_id);
        assert!(body["artist_links"].is_array());

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_show_not_found() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "artists").await;

        let req = test::TestRequest::get()
            .uri(&format!("/artist/{}", not_found))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_create_admin() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        let name = format!("New Artist {}", ts);

        let req = test::TestRequest::post()
            .uri("/artist")
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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_create_forbidden_non_admin() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let token = encode_token_with_role(2, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap();

        let req = test::TestRequest::post()
            .uri("/artist")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "name": "Should Fail"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 403);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_update_success() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "artist_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let body = artist_update_multipart_body(
            "Updated Artist Name",
            "test bio",
            "test subheading",
            "test intro",
        );

        let req = test::TestRequest::put()
            .uri(&format!("/artist/{}", artist_id))
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .insert_header(("Content-Type", "multipart/form-data; boundary=artist_boundary"))
            .set_payload(body)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let response_body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(response_body["name"], "Updated Artist Name");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_update_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let not_found = not_found_id(&state.db, "artists").await;

        let body = artist_update_multipart_body(
            "Updated",
            "test bio",
            "test subheading",
            "test intro",
        );

        let req = test::TestRequest::put()
            .uri(&format!("/artist/{}", not_found))
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .insert_header(("Content-Type", "multipart/form-data; boundary=artist_boundary"))
            .set_payload(body)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_update_multipart_fields() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "artist_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let body = artist_update_multipart_body(
            "Updated Name",
            "Updated Bio",
            "Updated Subheading",
            "Updated Intro",
        );

        let req = test::TestRequest::put()
            .uri(&format!("/artist/{}", artist_id))
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .insert_header(("Content-Type", "multipart/form-data; boundary=artist_boundary"))
            .set_payload(body)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let response_body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(response_body["name"], "Updated Name");
        assert_eq!(response_body["bio"], "Updated Bio");
        assert_eq!(response_body["sub_heading"], "Updated Subheading");
        assert_eq!(response_body["intro"], "Updated Intro");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_artist_link_success() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "artist_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let req = test::TestRequest::post()
            .uri("/artist/add_artist_links")
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
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

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn add_artist_link_invalid_url() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/artist/add_artist_links")
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .set_json(serde_json::json!({
                "artist_id": 1,
                "link_type": 1,
                "url": "not-a-url"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_artist_link_success() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "artist_test", "artist").await;
        let artist_id = seed_artist(&state.db, Some(user_id)).await;

        let now = chrono::Utc::now().naive_utc();
        let link_id: i64 = sqlx::query_scalar(
            r"INSERT INTO artist_links (artist_id, link_type, url, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING id",
        )
        .bind(artist_id)
        .bind(1)
        .bind("https://spotify.com/test")
        .bind(&now)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed artist link");

        let req = test::TestRequest::post()
            .uri("/artist/delete_artist_links")
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .set_json(serde_json::json!({
                "id": link_id
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let remaining: i64 = sqlx::query_scalar(r"SELECT COUNT(*) FROM artist_links WHERE id = $1")
            .bind(link_id)
            .fetch_one(&state.db)
            .await
            .expect("Failed to check link");
        assert_eq!(remaining, 0);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn delete_artist_link_not_found() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/artist/delete_artist_links")
            .insert_header(("Authorization", format!("Bearer {}", artist_token(2))))
            .set_json(serde_json::json!({
                "id": 99999999
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn available_link_types_public() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/available_link_types")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body.as_array().unwrap().len(), 11);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_sign_up_success() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = chrono::Utc::now().timestamp_micros();
        let email = format!("artistsignup{}@example.com", ts);

        let token = format!("artist_signup_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        let now = chrono::Utc::now().naive_utc();
        let _trigger_id: i64 = sqlx::query_scalar(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ($1, $2, $3, 'artist', $4, $4) RETURNING id",
        )
        .bind(&email)
        .bind(&token)
        .bind(&future)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed trigger");

        let req = test::TestRequest::post()
            .uri("/artist/sign_up")
            .set_json(serde_json::json!({
                "token": token,
                "name": "New Artist Band",
                "password": "TestPass1",
                "password_confirmation": "TestPass1",
                "username": "newartist"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["name"], "New Artist Band");

        let user_role: Option<String> =
            sqlx::query_scalar(r"SELECT role FROM users WHERE email = $1")
                .bind(&email)
                .fetch_one(&state.db)
                .await
                .expect("Failed to check user role");
        assert_eq!(user_role, Some("artist".to_string()));

        let user_username: Option<String> =
            sqlx::query_scalar(r"SELECT username FROM users WHERE email = $1")
                .bind(&email)
                .fetch_one(&state.db)
                .await
                .expect("Failed to check username");
        assert_eq!(user_username, Some("newartist".to_string()));

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_sign_up_invalid_token() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/artist/sign_up")
            .set_json(serde_json::json!({
                "token": "nonexistent_token",
                "name": "New Artist Band",
                "password": "TestPass1",
                "password_confirmation": "TestPass1"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_sign_up_duplicate_email_returns_409() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = chrono::Utc::now().timestamp_micros();
        let email = format!("artistdup{}@example.com", ts);

        // Seed an existing artist user
        let now = chrono::Utc::now().naive_utc();
        let existing_user_id: i64 = sqlx::query_scalar(
            r"INSERT INTO users (email, password_digest, role, created_at, updated_at)
               VALUES ($1, $2, 'artist', $3, $3) RETURNING id",
        )
        .bind(&email)
        .bind("hashed_password_test")
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed existing user");

        let token = format!("artist_dup_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        let _trigger_id: i64 = sqlx::query_scalar(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ($1, $2, $3, 'artist', $4, $4) RETURNING id",
        )
        .bind(&email)
        .bind(&token)
        .bind(&future)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed trigger");

        let req = test::TestRequest::post()
            .uri("/artist/sign_up")
            .set_json(serde_json::json!({
                "token": token,
                "name": "Duplicate Artist",
                "password": "TestPass1",
                "password_confirmation": "TestPass1"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 409);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_sign_up_sets_prospect_true() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = chrono::Utc::now().timestamp_micros();
        let email = format!("artistprospect{}@example.com", ts);

        let token = format!("artist_prospect_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        let now = chrono::Utc::now().naive_utc();
        let _trigger_id: i64 = sqlx::query_scalar(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ($1, $2, $3, 'artist', $4, $4) RETURNING id",
        )
        .bind(&email)
        .bind(&token)
        .bind(&future)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed trigger");

        let req = test::TestRequest::post()
            .uri("/artist/sign_up")
            .set_json(serde_json::json!({
                "token": token,
                "name": "Prospect Artist",
                "password": "TestPass1",
                "password_confirmation": "TestPass1"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let prospect: bool = sqlx::query_scalar(
            r"SELECT prospect FROM artists WHERE user_id IN (SELECT id FROM users WHERE email = $1)"
        )
        .bind(&email)
        .fetch_one(&state.db)
        .await
        .expect("Failed to check prospect flag");
        assert!(prospect, "Artist should have prospect = true");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_sign_up_expired_token() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = chrono::Utc::now().timestamp_micros();
        let email = format!("artistexpired{}@example.com", ts);

        let token = format!("artist_expired_token_{}", ts);
        let past = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
        let now = chrono::Utc::now().naive_utc();
        let _trigger_id: i64 = sqlx::query_scalar(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ($1, $2, $3, 'artist', $4, $4) RETURNING id",
        )
        .bind(&email)
        .bind(&token)
        .bind(&past)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed trigger");

        let req = test::TestRequest::post()
            .uri("/artist/sign_up")
            .set_json(serde_json::json!({
                "token": token,
                "name": "Expired Artist Band",
                "password": "TestPass1",
                "password_confirmation": "TestPass1"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_sign_up_password_mismatch() {
        crate::test_utils::set_test_env();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::post()
            .uri("/artist/sign_up")
            .set_json(serde_json::json!({
                "token": "any_token",
                "name": "Test Artist",
                "password": "TestPass1",
                "password_confirmation": "Different1"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_sign_up_weak_password() {
        crate::test_utils::set_test_env();
        let (_guard, state, app) = crate::build_test_app!(config_routes);
        let ts = chrono::Utc::now().timestamp_micros();
        let email = format!("artistweak{}@example.com", ts);

        let token = format!("artist_weak_token_{}", ts);
        let future = (chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339();
        let now = chrono::Utc::now().naive_utc();
        let _trigger_id: i64 = sqlx::query_scalar(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ($1, $2, $3, 'artist', $4, $4) RETURNING id",
        )
        .bind(&email)
        .bind(&token)
        .bind(&future)
        .bind(&now)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed trigger");

        let req = test::TestRequest::post()
            .uri("/artist/sign_up")
            .set_json(serde_json::json!({
                "token": token,
                "name": "Weak Artist",
                "password": "weak",
                "password_confirmation": "weak"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);

        _guard.cleanup().await;
    }

    fn artist_update_multipart_body(
        name: &str,
        bio: &str,
        sub_heading: &str,
        intro: &str,
    ) -> Vec<u8> {
        let boundary = "artist_boundary";
        let mut body = Vec::new();

        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"artist[name]\"\r\n\r\n{}\r\n",
            boundary, name
        ).as_bytes());

        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"artist[bio]\"\r\n\r\n{}\r\n",
            boundary, bio
        ).as_bytes());

        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"artist[subHeading]\"\r\n\r\n{}\r\n",
            boundary, sub_heading
        ).as_bytes());

        body.extend_from_slice(format!(
            "--{}\r\nContent-Disposition: form-data; name=\"artist[intro]\"\r\n\r\n{}\r\n",
            boundary, intro
        ).as_bytes());

        body.extend_from_slice(format!("\r\n--{}--\r\n", boundary).as_bytes());
        body
    }
}
