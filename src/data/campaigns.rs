use crate::models::campaign::Campaign;

const CAMPAIGN_COLUMNS: &str =
    r#"id, artist_id, name, active, vinyl_sold_count,
       campaign_start_date, campaign_end_date, progress, album_id,
       deleted_at, created_at, updated_at"#;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Campaign>, sqlx::Error> {
    sqlx::query_as::<_, Campaign>(
        &format!(r"SELECT {} FROM campaigns WHERE deleted_at IS NULL ORDER BY id", CAMPAIGN_COLUMNS),
    )
    .fetch_all(pool)
    .await
}

pub async fn list_by_artist(pool: &sqlx::PgPool, artist_id: i64) -> Result<Vec<Campaign>, sqlx::Error> {
    sqlx::query_as::<_, Campaign>(
        &format!(r"SELECT {} FROM campaigns WHERE artist_id = $1 AND deleted_at IS NULL ORDER BY id", CAMPAIGN_COLUMNS),
    )
    .bind(artist_id)
    .fetch_all(pool)
    .await
}

pub async fn active(pool: &sqlx::PgPool) -> Result<Vec<Campaign>, sqlx::Error> {
    sqlx::query_as::<_, Campaign>(
        &format!(r"SELECT {} FROM campaigns WHERE active = true AND deleted_at IS NULL ORDER BY id", CAMPAIGN_COLUMNS),
    )
    .fetch_all(pool)
    .await
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<Campaign>, sqlx::Error> {
    sqlx::query_as::<_, Campaign>(
        &format!(r"SELECT {} FROM campaigns WHERE id = $1", CAMPAIGN_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_active(pool: &sqlx::PgPool, id: i64) -> Result<Option<Campaign>, sqlx::Error> {
    sqlx::query_as::<_, Campaign>(
        &format!(r"SELECT {} FROM campaigns WHERE id = $1 AND deleted_at IS NULL", CAMPAIGN_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create(
    pool: &sqlx::PgPool,
    artist_id: i64,
    name: &str,
    vinyl_sold_count: Option<i32>,
    now: chrono::NaiveDateTime,
) -> Result<Campaign, sqlx::Error> {
    sqlx::query_as::<_, Campaign>(
        &format!(
            r#"INSERT INTO campaigns (artist_id, name, active, vinyl_sold_count, campaign_start_date, campaign_end_date, progress, album_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
               RETURNING {}"#,
            CAMPAIGN_COLUMNS
        ),
    )
    .bind(artist_id)
    .bind(name)
    .bind(false)
    .bind(vinyl_sold_count)
    .bind::<Option<chrono::NaiveDateTime>>(None)
    .bind::<Option<chrono::NaiveDateTime>>(None)
    .bind(0_i32)
    .bind::<Option<i64>>(None)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &sqlx::PgPool,
    id: i64,
    name: &Option<String>,
    active: Option<bool>,
    vinyl_sold_count: Option<i32>,
    progress: Option<i32>,
    campaign_start_date: Option<chrono::NaiveDateTime>,
    campaign_end_date: Option<chrono::NaiveDateTime>,
    now: chrono::NaiveDateTime,
) -> Result<Option<Campaign>, sqlx::Error> {
    sqlx::query_as::<_, Campaign>(
        &format!(
            r#"UPDATE campaigns SET
               name = COALESCE($1, name),
               active = COALESCE($2, active),
               vinyl_sold_count = COALESCE($3, vinyl_sold_count),
               progress = COALESCE($4, progress),
               campaign_start_date = COALESCE($5, campaign_start_date),
               campaign_end_date = COALESCE($6, campaign_end_date),
               updated_at = $7
           WHERE id = $8
           RETURNING {}"#,
            CAMPAIGN_COLUMNS
        ),
    )
    .bind(name)
    .bind(active)
    .bind(vinyl_sold_count)
    .bind(progress)
    .bind(campaign_start_date)
    .bind(campaign_end_date)
    .bind(now)
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn activate(
    pool: &sqlx::PgPool,
    id: i64,
    start_date: chrono::NaiveDateTime,
    end_date: chrono::NaiveDateTime,
    now: chrono::NaiveDateTime,
) -> Result<Campaign, sqlx::Error> {
    sqlx::query_as::<_, Campaign>(
        &format!(
            r#"UPDATE campaigns SET
               active = true,
               campaign_start_date = $1,
               campaign_end_date = $2,
               updated_at = $3
           WHERE id = $4
           RETURNING {}"#,
            CAMPAIGN_COLUMNS
        ),
    )
    .bind(start_date)
    .bind(end_date)
    .bind(now)
    .bind(id)
    .fetch_one(pool)
    .await
}

pub async fn destroy(pool: &sqlx::PgPool, id: i64, now: chrono::NaiveDateTime) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"UPDATE campaigns SET deleted_at = $1, updated_at = $2
            WHERE id = $3 AND deleted_at IS NULL"#,
    )
    .bind(now)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn update_progress(
    pool: &sqlx::PgPool,
    id: i64,
    vinyl_sold_count: i32,
    progress: i32,
    now: chrono::NaiveDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE campaigns SET vinyl_sold_count = $1, progress = $2, updated_at = $3 WHERE id = $4"#,
    )
    .bind(vinyl_sold_count)
    .bind(progress)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}
