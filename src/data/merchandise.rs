use crate::models::artist_merchandise::ArtistMerchandiseWithCache;
use crate::models::shopify_json_cache::ShopifyJsonCache;

const MERCH_SELECT: &str = r"SELECT am.id, am.artist_id, am.producer_id, am.merchandise_id, am.description,
    am.created_on_producer, am.merch_title, am.merch_product_title, am.set_price::float8, am.cost_price::float8,
    am.created_at, am.updated_at, sjc.json_entry
    FROM artist_merchandise am
    LEFT JOIN shopify_json_caches sjc ON sjc.id::text = am.merchandise_id";

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<ArtistMerchandiseWithCache>, sqlx::Error> {
    sqlx::query_as::<_, ArtistMerchandiseWithCache>(
        &format!("{} ORDER BY am.id", MERCH_SELECT),
    )
    .fetch_all(pool)
    .await
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<ArtistMerchandiseWithCache>, sqlx::Error> {
    sqlx::query_as::<_, ArtistMerchandiseWithCache>(
        &format!("{} WHERE am.id = $1", MERCH_SELECT),
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn by_artist(pool: &sqlx::PgPool, artist_id: i64) -> Result<Vec<ArtistMerchandiseWithCache>, sqlx::Error> {
    sqlx::query_as::<_, ArtistMerchandiseWithCache>(
        &format!("{} WHERE am.artist_id = $1 ORDER BY am.id", MERCH_SELECT),
    )
    .bind(artist_id)
    .fetch_all(pool)
    .await
}

pub async fn create(
    pool: &sqlx::PgPool,
    artist_id: i64,
    producer_id: i64,
    merchandise_id: &Option<String>,
    description: &Option<String>,
    created_on_producer: Option<bool>,
    merch_title: &str,
    merch_product_title: &Option<String>,
    set_price: Option<f64>,
    cost_price: Option<f64>,
    now: chrono::NaiveDateTime,
) -> Result<ArtistMerchandiseWithCache, sqlx::Error> {
    sqlx::query_as::<_, ArtistMerchandiseWithCache>(
        "INSERT INTO artist_merchandise (artist_id, producer_id, merchandise_id, description,
         created_on_producer, merch_title, merch_product_title, set_price, cost_price,
         created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8::numeric, $9::numeric, $10, $11)
         RETURNING id, artist_id, producer_id, merchandise_id, description,
         created_on_producer, merch_title, merch_product_title, set_price::float8, cost_price::float8,
         created_at, updated_at,
         (SELECT sjc.json_entry FROM shopify_json_caches sjc WHERE sjc.id::text = artist_merchandise.merchandise_id)",
    )
    .bind(artist_id)
    .bind(producer_id)
    .bind(merchandise_id)
    .bind(description)
    .bind(created_on_producer)
    .bind(merch_title)
    .bind(merch_product_title)
    .bind(set_price)
    .bind(cost_price)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &sqlx::PgPool,
    id: i64,
    merch_title: Option<&str>,
    merch_product_title: Option<&str>,
    description: Option<&str>,
    set_price: Option<f64>,
    cost_price: Option<f64>,
    now: chrono::NaiveDateTime,
) -> Result<Option<ArtistMerchandiseWithCache>, sqlx::Error> {
    sqlx::query_as::<_, ArtistMerchandiseWithCache>(
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
         created_at, updated_at,
         (SELECT sjc.json_entry FROM shopify_json_caches sjc WHERE sjc.id::text = artist_merchandise.merchandise_id)",
    )
    .bind(merch_title)
    .bind(merch_product_title)
    .bind(description)
    .bind(set_price)
    .bind(cost_price)
    .bind(now)
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn destroy(pool: &sqlx::PgPool, id: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(r"DELETE FROM artist_merchandise WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn cache_update(pool: &sqlx::PgPool) -> Result<Vec<ShopifyJsonCache>, sqlx::Error> {
    sqlx::query_as::<_, ShopifyJsonCache>(
        r"SELECT id, cached_item_id, json_entry, created_at, updated_at FROM shopify_json_caches ORDER BY id",
    )
    .fetch_all(pool)
    .await
}
