use chrono::NaiveDate;
use sqlx::FromRow;

use crate::models::album::Album;

#[derive(Debug, FromRow)]
pub struct AlbumRow {
    pub id: i64,
    pub name: Option<String>,
    pub release_date: NaiveDate,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
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

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Album>, sqlx::Error> {
    let rows = sqlx::query_as::<_, AlbumRow>(
        r"SELECT id, name, release_date, created_at, updated_at FROM albums ORDER BY id",
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<Album>, sqlx::Error> {
    let row = sqlx::query_as::<_, AlbumRow>(
        r"SELECT id, name, release_date, created_at, updated_at FROM albums WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn create(
    pool: &sqlx::PgPool,
    name: &str,
    release_date: Option<NaiveDate>,
    now: chrono::NaiveDateTime,
) -> Result<Album, sqlx::Error> {
    let row = sqlx::query_as::<_, AlbumRow>(
        r"INSERT INTO albums (name, release_date, created_at, updated_at)
           VALUES ($1, $2, $3, $4)
           RETURNING id, name, release_date, created_at, updated_at",
    )
    .bind(name)
    .bind(release_date)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}
