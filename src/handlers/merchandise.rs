//! Merchandise handlers
//!
//! Provides CRUD endpoints for artist merchandise management and a Shopify
//! cache retrieval endpoint. All endpoints require authentication.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `index` | GET | `/v1/merchandise` or `/v1/artist_merchandise` | auth | List all merchandise items |
//! | `show` | GET | `/v1/artist_merchandise/{id}` | auth | Retrieve a single merchandise item by ID |
//! | `by_artist` | GET | `/v1/artist_merchandise/by_artist/{artist_id}` | auth | List merchandise for a specific artist |
//! | `create` | POST | `/v1/artist_merchandise` | auth | Create a new merchandise item |
//! | `update` | PUT | `/v1/artist_merchandise/{id}` | auth | Update an existing merchandise item |
//! | `destroy` | DELETE | `/v1/artist_merchandise/{id}` | auth | Delete a merchandise item |
//! | `cache_update` | GET | `/v1/merchandise/cache_update` | auth | Retrieve Shopify JSON cache entries |

use actix_web::{web, HttpResponse};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::artist_merchandise::{
    ArtistMerchandise, ArtistMerchandiseResponse, CreateArtistMerchandiseRequest,
    UpdateArtistMerchandiseRequest,
};
use crate::models::shopify_json_cache::{ShopifyJsonCache, ShopifyJsonCacheResponse};

#[derive(Debug, FromRow)]
struct ArtistMerchandiseRow {
    id: i64,
    artist_id: i64,
    producer_id: i64,
    merchandise_id: Option<String>,
    description: Option<String>,
    created_on_producer: Option<bool>,
    merch_title: String,
    merch_product_title: Option<String>,
    set_price: Option<f64>,
    cost_price: Option<f64>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<ArtistMerchandiseRow> for ArtistMerchandise {
    fn from(row: ArtistMerchandiseRow) -> Self {
        ArtistMerchandise {
            id: row.id,
            artist_id: row.artist_id,
            producer_id: row.producer_id,
            merchandise_id: row.merchandise_id,
            description: row.description,
            created_on_producer: row.created_on_producer,
            merch_title: row.merch_title,
            merch_product_title: row.merch_product_title,
            set_price: row.set_price,
            cost_price: row.cost_price,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

#[derive(Debug, FromRow)]
struct ShopifyJsonCacheRow {
    id: i64,
    cached_item_id: Option<String>,
    json_entry: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<ShopifyJsonCacheRow> for ShopifyJsonCache {
    fn from(row: ShopifyJsonCacheRow) -> Self {
        ShopifyJsonCache {
            id: row.id,
            cached_item_id: row.cached_item_id,
            json_entry: row.json_entry,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const MERCH_SELECT: &str = r"SELECT id, artist_id, producer_id, merchandise_id, description,
    created_on_producer, merch_title, merch_product_title, set_price::float8, cost_price::float8,
    created_at, updated_at FROM artist_merchandise";

/// List all merchandise items.
///
/// Returns all merchandise ordered by ID. Requires authentication.
///
/// # Response
///
/// `200 OK` — JSON array of `ArtistMerchandiseResponse`
pub async fn index(
    _user: CurrentUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let rows = sqlx::query_as::<_, ArtistMerchandiseRow>(
        &format!("{} ORDER BY id", MERCH_SELECT)
    )
    .fetch_all(&state.db)
    .await?;

    let items: Vec<ArtistMerchandise> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<ArtistMerchandiseResponse> = items.iter().map(|m| m.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Retrieve a single merchandise item by ID.
///
/// Requires authentication.
///
/// # Response
///
/// `200 OK` — `ArtistMerchandiseResponse`
/// `404 Not Found` — merchandise item does not exist
pub async fn show(
    _user: CurrentUser,
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    match sqlx::query_as::<_, ArtistMerchandiseRow>(
        &format!("{} WHERE id = $1", MERCH_SELECT)
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let merch: ArtistMerchandise = row.into();
            Ok(HttpResponse::Ok().json(merch.to_response()))
        }
        None => Err(AppError::NotFound(format!("Merchandise #{}", id))),
    }
}

/// List merchandise for a specific artist.
///
/// Returns all merchandise items associated with the given artist ID.
/// Requires authentication.
///
/// # Response
///
/// `200 OK` — JSON array of `ArtistMerchandiseResponse`
pub async fn by_artist(
    _user: CurrentUser,
    state: web::Data<AppState>,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let artist_id = path.into_inner();

    let rows = sqlx::query_as::<_, ArtistMerchandiseRow>(
        &format!("{} WHERE artist_id = $1 ORDER BY id", MERCH_SELECT)
    )
    .bind(artist_id)
    .fetch_all(&state.db)
    .await?;

    let items: Vec<ArtistMerchandise> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<ArtistMerchandiseResponse> = items.iter().map(|m| m.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Create a new merchandise item.
///
/// Requires authentication. Validates that `merch_title` is non-empty.
///
/// # Response
///
/// `201 Created` — `ArtistMerchandiseResponse`
/// `422 Unprocessable` — merch_title is empty
pub async fn create(
    state: web::Data<AppState>,
    _user: CurrentUser,
    body: web::Json<CreateArtistMerchandiseRequest>,
) -> Result<HttpResponse, AppError> {
    if body.merch_title.is_empty() {
        return Err(AppError::Validation("merch_title is required".to_string()));
    }

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, ArtistMerchandiseRow>(
        "INSERT INTO artist_merchandise (artist_id, producer_id, merchandise_id, description,
         created_on_producer, merch_title, merch_product_title, set_price, cost_price,
         created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::numeric, $9::numeric, $10, $11)
         RETURNING id, artist_id, producer_id, merchandise_id, description,
         created_on_producer, merch_title, merch_product_title, set_price::float8, cost_price::float8,
         created_at, updated_at"
    )
    .bind(body.artist_id)
    .bind(body.producer_id)
    .bind(&body.merchandise_id)
    .bind(&body.description)
    .bind(body.created_on_producer)
    .bind(&body.merch_title)
    .bind(&body.merch_product_title)
    .bind(body.set_price)
    .bind(body.cost_price)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let merch: ArtistMerchandise = row.into();
    Ok(HttpResponse::Created().json(merch.to_response()))
}

/// Update an existing merchandise item.
///
/// Requires authentication. Uses COALESCE for partial updates of title,
/// product title, description, set price, and cost price.
///
/// # Response
///
/// `200 OK` — `ArtistMerchandiseResponse`
/// `404 Not Found` — merchandise item does not exist
pub async fn update(
    state: web::Data<AppState>,
    _user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateArtistMerchandiseRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    let title = body.merch_title.clone();
    let product_title = body.merch_product_title.clone();
    let desc = body.description.clone();
    let set_price = body.set_price;
    let cost_price = body.cost_price;
    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, ArtistMerchandiseRow>(
        "UPDATE artist_merchandise
         SET merch_title = COALESCE($1, merch_title),
             merch_product_title = COALESCE($2, merch_product_title),
             description = COALESCE($3, description),
             set_price = COALESCE($4::numeric, set_price),
             cost_price = COALESCE($5::numeric, cost_price),
             updated_at = $6
         WHERE id = $7
         RETURNING id, artist_id, producer_id, merchandise_id, description,
         created_on_producer, merch_title, merch_product_title, set_price::float8, cost_price::float8,
         created_at, updated_at"
    )
    .bind(title.as_deref())
    .bind(product_title.as_deref())
    .bind(desc.as_deref())
    .bind(set_price)
    .bind(cost_price)
    .bind(now)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(r) => {
            let merch: ArtistMerchandise = r.into();
            Ok(HttpResponse::Ok().json(merch.to_response()))
        }
        None => Err(AppError::NotFound(format!("Merchandise #{}", id))),
    }
}

/// Delete a merchandise item.
///
/// Requires authentication. Permanently removes the row from the database.
///
/// # Response
///
/// `200 OK` — JSON object with confirmation message
/// `404 Not Found` — merchandise item does not exist
pub async fn destroy(
    state: web::Data<AppState>,
    _user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    let result = sqlx::query(r"DELETE FROM artist_merchandise WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(format!("Merchandise #{}", id)));
    }

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Merchandise #{} deleted", id)
    })))
}

/// Retrieve Shopify JSON cache entries.
///
/// Returns all cached Shopify JSON entries ordered by ID.
/// Requires authentication.
///
/// # Response
///
/// `200 OK` — JSON array of `ShopifyJsonCacheResponse`
pub async fn cache_update(
    _user: CurrentUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let rows = sqlx::query_as::<_, ShopifyJsonCacheRow>(
        r"SELECT id, cached_item_id, json_entry, created_at, updated_at FROM shopify_json_caches ORDER BY id"
    )
    .fetch_all(&state.db)
    .await?;

    let caches: Vec<ShopifyJsonCache> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<ShopifyJsonCacheResponse> = caches.iter().map(|c| c.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/merchandise", web::get().to(index))
        .route("/merchandise/cache_update", web::get().to(cache_update))
        .route("/artist_merchandise", web::get().to(index))
        .route("/artist_merchandise/{id}", web::get().to(show))
        .route("/artist_merchandise/by_artist/{artist_id}", web::get().to(by_artist))
        .route("/artist_merchandise", web::post().to(create))
        .route("/artist_merchandise/{id}", web::put().to(update))
        .route("/artist_merchandise/{id}", web::delete().to(destroy));
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

    async fn seed_artist_and_producer(pool: &sqlx::PgPool) -> (i64, i64, String, String) {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let artist_name = format!("Test Artist Merch {}", suffix);
        let producer_name = format!("Test Producer Merch {}", suffix);

        let artist_id: i64 = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO artists (name, created_at, updated_at)
               VALUES ($1, NOW(), NOW())
               RETURNING id"
        )
        .bind(&artist_name)
        .fetch_one(pool)
        .await
        .expect("Failed to seed artist");

        let producer_id: i64 = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO producers (producer_name, description, created_at, updated_at)
               VALUES ($1, 'Test producer', NOW(), NOW())
               RETURNING id"
        )
        .bind(&producer_name)
        .fetch_one(pool)
        .await
        .expect("Failed to seed producer");

        (artist_id, producer_id, artist_name, producer_name)
    }

    async fn cleanup_by_artist_name(pool: &sqlx::PgPool, artist_name: &str, producer_name: &str) {
        let _ = sqlx::query(r"DELETE FROM artists WHERE name = $1")
            .bind(artist_name)
            .execute(pool)
            .await;
        let _ = sqlx::query(r"DELETE FROM producers WHERE producer_name = $1")
            .bind(producer_name)
            .execute(pool)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_index_authenticated() {
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
            .uri("/artist_merchandise")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_show_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let (artist_id, producer_id, artist_name, producer_name) = seed_artist_and_producer(&state.db).await;

        let seed = sqlx::query_as::<_, ArtistMerchandiseRow>(
            r"INSERT INTO artist_merchandise (artist_id, producer_id, merchandise_id, description,
               created_on_producer, merch_title, merch_product_title, set_price, cost_price,
               created_at, updated_at)
               VALUES ($1, $2, 'shopify-test', 'Band Tee', true, 'Band T-shirt', 'Band Tee - Black', 25.99, 12.50, NOW(), NOW())
               RETURNING id, artist_id, producer_id, merchandise_id, description,
               created_on_producer, merch_title, merch_product_title, set_price::float8, cost_price::float8,
               created_at, updated_at"
        )
        .bind(artist_id)
        .bind(producer_id)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed merchandise");

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let id = seed.id;
        let req = test::TestRequest::get()
            .uri(&format!("/artist_merchandise/{}", id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["merch_title"], "Band T-shirt");

        let _ = sqlx::query(&format!(r"DELETE FROM artist_merchandise WHERE id = {}", id))
            .execute(&state.db)
            .await;
        let _ = cleanup_by_artist_name(&state.db, &artist_name, &producer_name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_show_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM artist_merchandise"
        )
        .fetch_one(&state.db)
        .await
        .expect("Failed to get max id");

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/artist_merchandise/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_by_artist() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let (artist_id, producer_id, artist_name, producer_name) = seed_artist_and_producer(&state.db).await;

        let _ = sqlx::query(
            r"INSERT INTO artist_merchandise (artist_id, producer_id, merchandise_id, merch_title, created_at, updated_at)
               VALUES ($1, $2, 'shopify-by-artist', 'By Artist Tee', NOW(), NOW())"
        )
        .bind(artist_id)
        .bind(producer_id)
        .execute(&state.db)
        .await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/artist_merchandise/by_artist/{}", artist_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert!(!body.is_empty());
        assert_eq!(body[0]["merch_title"], "By Artist Tee");

        let _ = sqlx::query(r"DELETE FROM artist_merchandise WHERE merchandise_id = 'shopify-by-artist'")
            .execute(&state.db)
            .await;
        let _ = cleanup_by_artist_name(&state.db, &artist_name, &producer_name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_create_success() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let (artist_id, producer_id, artist_name, producer_name) = seed_artist_and_producer(&state.db).await;

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let title = format!("New Item {}", suffix);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/artist_merchandise")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "artist_id": artist_id,
                "producer_id": producer_id,
                "merch_title": title,
                "description": "Test description",
                "set_price": 19.99
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(status, 201, "Expected 201, got {}: {:?}", status, body);
        assert_eq!(body["merch_title"], title);
        assert_eq!(body["set_price"], 19.99);

        let _ = sqlx::query(r"DELETE FROM artist_merchandise WHERE merch_title = $1")
            .bind(&title)
            .execute(&state.db)
            .await;
        let _ = cleanup_by_artist_name(&state.db, &artist_name, &producer_name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_create_empty_title() {
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
            .uri("/artist_merchandise")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "artist_id": 1,
                "producer_id": 1,
                "merch_title": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_update_success() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let (artist_id, producer_id, artist_name, producer_name) = seed_artist_and_producer(&state.db).await;

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let original_title = format!("Original {}", suffix);
        let new_title = format!("Updated {}", suffix);

        let id: i64 = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO artist_merchandise (artist_id, producer_id, merch_title, description, created_at, updated_at)
               VALUES ($1, $2, $3, 'Original desc', NOW(), NOW())
               RETURNING id"
        )
        .bind(artist_id)
        .bind(producer_id)
        .bind(&original_title)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed merchandise");

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/artist_merchandise/{}", id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "merch_title": new_title,
                "description": "Updated desc",
                "set_price": 29.99
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["merch_title"], new_title);
        assert_eq!(body["description"], "Updated desc");
        assert_eq!(body["set_price"], 29.99);

        let _ = sqlx::query(&format!(r"DELETE FROM artist_merchandise WHERE id = {}", id))
            .execute(&state.db)
            .await;
        let _ = cleanup_by_artist_name(&state.db, &artist_name, &producer_name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_update_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM artist_merchandise"
        )
        .fetch_one(&state.db)
        .await
        .expect("Failed to get max id");

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/artist_merchandise/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "merch_title": "Updated"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_destroy_success() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let (artist_id, producer_id, artist_name, producer_name) = seed_artist_and_producer(&state.db).await;

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let title = format!("Delete Me {}", suffix);

        let id: i64 = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO artist_merchandise (artist_id, producer_id, merch_title, created_at, updated_at)
               VALUES ($1, $2, $3, NOW(), NOW())
               RETURNING id"
        )
        .bind(artist_id)
        .bind(producer_id)
        .bind(&title)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed merchandise");

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/artist_merchandise/{}", id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["message"], format!("Merchandise #{} deleted", id));
        let _ = cleanup_by_artist_name(&state.db, &artist_name, &producer_name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_destroy_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);

        let max_id: i64 = sqlx::query_scalar(
            r"SELECT COALESCE(MAX(id), 0) FROM artist_merchandise"
        )
        .fetch_one(&state.db)
        .await
        .expect("Failed to get max id");

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(&format!("/artist_merchandise/{}", max_id + 9999))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn cache_update_returns_ok() {
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
            .uri("/merchandise/cache_update")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert!(body.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn by_artist_returns_empty_when_no_merchandise() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let (artist_id, _producer_id, artist_name, producer_name) = seed_artist_and_producer(&state.db).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/artist_merchandise/by_artist/{}", artist_id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert!(body.is_empty());

        let _ = cleanup_by_artist_name(&state.db, &artist_name, &producer_name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_unauthenticated_returns_401() {
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
            .uri("/artist_merchandise")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_update_partial_preserves_unsent_fields() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let (artist_id, producer_id, artist_name, producer_name) = seed_artist_and_producer(&state.db).await;

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let original_title = format!("Partial {}", suffix);

        let id: i64 = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO artist_merchandise (artist_id, producer_id, merch_title, description, set_price, cost_price, created_at, updated_at)
               VALUES ($1, $2, $3, 'Original desc', 19.99, 8.50, NOW(), NOW())
               RETURNING id"
        )
        .bind(artist_id)
        .bind(producer_id)
        .bind(&original_title)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed merchandise");

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/artist_merchandise/{}", id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "description": "Updated desc only"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["merch_title"], original_title);
        assert_eq!(body["description"], "Updated desc only");
        assert_eq!(body["set_price"], 19.99);
        assert_eq!(body["cost_price"], 8.50);

        let _ = sqlx::query(&format!(r"DELETE FROM artist_merchandise WHERE id = {}", id))
            .execute(&state.db)
            .await;
        let _ = cleanup_by_artist_name(&state.db, &artist_name, &producer_name).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_create_all_fields() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let (artist_id, producer_id, artist_name, producer_name) = seed_artist_and_producer(&state.db).await;

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let title = format!("Full Item {}", suffix);

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/artist_merchandise")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "artist_id": artist_id,
                "producer_id": producer_id,
                "merchandise_id": "shopify-full-test",
                "description": "Full test description",
                "created_on_producer": true,
                "merch_title": title,
                "merch_product_title": "Product Title",
                "set_price": 34.99,
                "cost_price": 15.00
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["merch_title"], title);
        assert_eq!(body["merchandise_id"], "shopify-full-test");
        assert_eq!(body["created_on_producer"], true);
        assert_eq!(body["merch_product_title"], "Product Title");
        assert_eq!(body["set_price"], 34.99);
        assert_eq!(body["cost_price"], 15.00);

        let _ = sqlx::query(r"DELETE FROM artist_merchandise WHERE merchandise_id = 'shopify-full-test'")
            .execute(&state.db)
            .await;
        let _ = cleanup_by_artist_name(&state.db, &artist_name, &producer_name).await;
    }
}
