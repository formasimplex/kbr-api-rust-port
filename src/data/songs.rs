use crate::models::song::Song;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Song>, sqlx::Error> {
    sqlx::query_as::<_, Song>(
        r"SELECT id, name, duration, album_id, artist_id, created_at, updated_at FROM songs ORDER BY id"
    )
    .fetch_all(pool)
    .await
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<Song>, sqlx::Error> {
    sqlx::query_as::<_, Song>(
        r"SELECT id, name, duration, album_id, artist_id, created_at, updated_at FROM songs WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create(
    pool: &sqlx::PgPool,
    name: &str,
    duration: &Option<String>,
    album_id: i64,
    artist_id: i64,
    now: chrono::NaiveDateTime,
) -> Result<Song, sqlx::Error> {
    sqlx::query_as::<_, Song>(
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
    .await
}
