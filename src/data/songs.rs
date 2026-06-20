use sqlx::FromRow;

use crate::models::song::Song;

#[derive(Debug, FromRow)]
pub struct SongRow {
    pub id: i64,
    pub name: Option<String>,
    pub duration: Option<String>,
    pub album_id: i64,
    pub artist_id: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
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

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Song>, sqlx::Error> {
    let rows = sqlx::query_as::<_, SongRow>(
        r"SELECT id, name, duration, album_id, artist_id, created_at, updated_at FROM songs ORDER BY id"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<Song>, sqlx::Error> {
    let row = sqlx::query_as::<_, SongRow>(
        r"SELECT id, name, duration, album_id, artist_id, created_at, updated_at FROM songs WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn create(
    pool: &sqlx::PgPool,
    name: &str,
    duration: &Option<String>,
    album_id: i64,
    artist_id: i64,
    now: chrono::NaiveDateTime,
) -> Result<Song, sqlx::Error> {
    let row = sqlx::query_as::<_, SongRow>(
        r"INSERT INTO songs (name, duration, album_id, artist_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, name, duration, album_id, artist_id, created_at, updated_at"
    )
    .bind(name)
    .bind(duration)
    .bind(album_id)
    .bind(artist_id)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}
