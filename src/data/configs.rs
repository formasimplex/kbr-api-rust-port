use uuid::Uuid;

use crate::models::tenant_config::TenantConfig;

const CONFIG_COLUMNS: &str = r#"tenant_id, logo_url, short_name, long_name, footer_logo_url, contact_email,
   site_header_description, deleted_at, "instaUrl", "twitterUrl", "tiktokUrl", "spotifyId",
   featured_artist_id, mantine_theme, created_at, updated_at"#;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<TenantConfig>, sqlx::Error> {
    sqlx::query_as::<_, TenantConfig>(
        &format!(r#"SELECT {} FROM tenant_configs WHERE deleted_at IS NULL"#, CONFIG_COLUMNS),
    )
    .fetch_all(pool)
    .await
}

pub async fn by_tenant_id(pool: &sqlx::PgPool, tenant_id: Uuid) -> Result<Option<TenantConfig>, sqlx::Error> {
    sqlx::query_as::<_, TenantConfig>(
        &format!(r#"SELECT {} FROM tenant_configs WHERE tenant_id = $1 AND deleted_at IS NULL"#, CONFIG_COLUMNS),
    )
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn create(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    logo_url: &Option<String>,
    short_name: &str,
    long_name: &str,
    footer_logo_url: &Option<String>,
    contact_email: &str,
    site_header_description: &str,
    insta_url: &Option<String>,
    twitter_url: &Option<String>,
    tiktok_url: &Option<String>,
    spotify_id: &Option<String>,
    featured_artist_id: Option<i64>,
    now: chrono::NaiveDateTime,
) -> Result<TenantConfig, sqlx::Error> {
    sqlx::query_as::<_, TenantConfig>(
        &format!(
            r#"INSERT INTO tenant_configs (tenant_id, logo_url, short_name, long_name, footer_logo_url,
               contact_email, site_header_description, deleted_at, "instaUrl", "twitterUrl", "tiktokUrl",
               "spotifyId", featured_artist_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, $8, $9, $10, $11, $12, $13, $14)
               RETURNING {}"#,
            CONFIG_COLUMNS
        ),
    )
    .bind(tenant_id)
    .bind(logo_url)
    .bind(short_name)
    .bind(long_name)
    .bind(footer_logo_url)
    .bind(contact_email)
    .bind(site_header_description)
    .bind(insta_url)
    .bind(twitter_url)
    .bind(tiktok_url)
    .bind(spotify_id)
    .bind(featured_artist_id)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    logo_url: &Option<String>,
    short_name: &Option<String>,
    long_name: &Option<String>,
    footer_logo_url: &Option<String>,
    contact_email: &Option<String>,
    site_header_description: &Option<String>,
    insta_url: &Option<String>,
    twitter_url: &Option<String>,
    tiktok_url: &Option<String>,
    spotify_id: &Option<String>,
    featured_artist_id: Option<i64>,
    now: chrono::NaiveDateTime,
) -> Result<Option<TenantConfig>, sqlx::Error> {
    sqlx::query_as::<_, TenantConfig>(
        &format!(
            r#"UPDATE tenant_configs
               SET logo_url = COALESCE($1, logo_url),
                   short_name = COALESCE($2, short_name),
                   long_name = COALESCE($3, long_name),
                   footer_logo_url = COALESCE($4, footer_logo_url),
                   contact_email = COALESCE($5, contact_email),
                   site_header_description = COALESCE($6, site_header_description),
                   "instaUrl" = COALESCE($7, "instaUrl"),
                   "twitterUrl" = COALESCE($8, "twitterUrl"),
                   "tiktokUrl" = COALESCE($9, "tiktokUrl"),
                   "spotifyId" = COALESCE($10, "spotifyId"),
                   featured_artist_id = COALESCE($11, featured_artist_id),
                   updated_at = $12
               WHERE tenant_id = $13 AND deleted_at IS NULL
               RETURNING {}"#,
            CONFIG_COLUMNS
        ),
    )
    .bind(logo_url)
    .bind(short_name)
    .bind(long_name)
    .bind(footer_logo_url)
    .bind(contact_email)
    .bind(site_header_description)
    .bind(insta_url)
    .bind(twitter_url)
    .bind(tiktok_url)
    .bind(spotify_id)
    .bind(featured_artist_id)
    .bind(now)
    .bind(tenant_id)
    .fetch_optional(pool)
    .await
}

pub async fn destroy(pool: &sqlx::PgPool, tenant_id: Uuid, now: chrono::NaiveDateTime) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r"UPDATE tenant_configs SET deleted_at = $1 WHERE tenant_id = $2 AND deleted_at IS NULL"
    )
    .bind(now)
    .bind(tenant_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}
