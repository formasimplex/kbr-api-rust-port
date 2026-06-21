use sqlx::FromRow;

use crate::models::campaign::Campaign;

#[derive(Debug, FromRow)]
pub struct CampaignRow {
    pub id: i64,
    pub artist_id: i64,
    pub name: Option<String>,
    pub active: Option<bool>,
    pub vinyl_sold_count: Option<i32>,
    pub campaign_start_date: Option<chrono::NaiveDateTime>,
    pub campaign_end_date: Option<chrono::NaiveDateTime>,
    pub progress: Option<i32>,
    pub album_id: Option<i64>,
    pub deleted_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<CampaignRow> for Campaign {
    fn from(row: CampaignRow) -> Self {
        Campaign {
            id: row.id,
            artist_id: row.artist_id,
            name: row.name,
            active: row.active,
            vinyl_sold_count: row.vinyl_sold_count,
            campaign_start_date: row.campaign_start_date.map(|dt| dt.and_utc()),
            campaign_end_date: row.campaign_end_date.map(|dt| dt.and_utc()),
            progress: row.progress,
            album_id: row.album_id,
            deleted_at: row.deleted_at.map(|dt| dt.and_utc()),
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const CAMPAIGN_COLUMNS: &str =
    r#"id, artist_id, name, active, vinyl_sold_count,
       campaign_start_date, campaign_end_date, progress, album_id,
       deleted_at, created_at, updated_at"#;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Campaign>, sqlx::Error> {
    let rows = sqlx::query_as::<_, CampaignRow>(
        &format!(r"SELECT {} FROM campaigns WHERE deleted_at IS NULL ORDER BY id", CAMPAIGN_COLUMNS),
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn list_by_artist(pool: &sqlx::PgPool, artist_id: i64) -> Result<Vec<Campaign>, sqlx::Error> {
    let rows = sqlx::query_as::<_, CampaignRow>(
        &format!(r"SELECT {} FROM campaigns WHERE artist_id = $1 AND deleted_at IS NULL ORDER BY id", CAMPAIGN_COLUMNS),
    )
    .bind(artist_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn active(pool: &sqlx::PgPool) -> Result<Vec<Campaign>, sqlx::Error> {
    let rows = sqlx::query_as::<_, CampaignRow>(
        &format!(r"SELECT {} FROM campaigns WHERE active = true AND deleted_at IS NULL ORDER BY id", CAMPAIGN_COLUMNS),
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<Campaign>, sqlx::Error> {
    let row = sqlx::query_as::<_, CampaignRow>(
        &format!(r"SELECT {} FROM campaigns WHERE id = $1", CAMPAIGN_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn find_active(pool: &sqlx::PgPool, id: i64) -> Result<Option<CampaignRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, CampaignRow>(
        &format!(r"SELECT {} FROM campaigns WHERE id = $1 AND deleted_at IS NULL", CAMPAIGN_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn create(
    pool: &sqlx::PgPool,
    artist_id: i64,
    name: &str,
    vinyl_sold_count: Option<i32>,
    now: chrono::NaiveDateTime,
) -> Result<Campaign, sqlx::Error> {
    let row = sqlx::query_as::<_, CampaignRow>(
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
    .await?;

    Ok(row.into())
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
    let row = sqlx::query_as::<_, CampaignRow>(
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
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn activate(
    pool: &sqlx::PgPool,
    id: i64,
    start_date: chrono::NaiveDateTime,
    end_date: chrono::NaiveDateTime,
    now: chrono::NaiveDateTime,
) -> Result<Campaign, sqlx::Error> {
    let row = sqlx::query_as::<_, CampaignRow>(
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
    .await?;

    Ok(row.into())
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
