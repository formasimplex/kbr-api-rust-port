use crate::models::news_playlist::NewsPlaylist;

const PLAYLIST_COLUMNS: &str = "id, user_id, name, description, created_at, updated_at";

pub async fn list(pool: &sqlx::PgPool, user_id: Option<i64>) -> Result<Vec<NewsPlaylist>, sqlx::Error> {
    let rows = if let Some(uid) = user_id {
        sqlx::query_as::<_, NewsPlaylist>(
            &format!(r"SELECT {} FROM news_playlists WHERE user_id = $1 ORDER BY id", PLAYLIST_COLUMNS),
        )
        .bind(uid)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, NewsPlaylist>(
            &format!(r"SELECT {} FROM news_playlists ORDER BY id", PLAYLIST_COLUMNS),
        )
        .fetch_all(pool)
        .await?
    };

    Ok(rows)
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<NewsPlaylist>, sqlx::Error> {
    sqlx::query_as::<_, NewsPlaylist>(
        &format!(r"SELECT {} FROM news_playlists WHERE id = $1", PLAYLIST_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create(
    pool: &sqlx::PgPool,
    user_id: i64,
    name: &str,
    description: &Option<String>,
    now: chrono::NaiveDateTime,
) -> Result<NewsPlaylist, sqlx::Error> {
    sqlx::query_as::<_, NewsPlaylist>(
        &format!(
            r"INSERT INTO news_playlists (user_id, name, description, created_at, updated_at)
              VALUES ($1, $2, $3, $4, $5)
              RETURNING {}",
            PLAYLIST_COLUMNS
        ),
    )
    .bind(user_id)
    .bind(name)
    .bind(description)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &sqlx::PgPool,
    id: i64,
    name: &Option<String>,
    description: &Option<String>,
    now: chrono::NaiveDateTime,
) -> Result<Option<NewsPlaylist>, sqlx::Error> {
    sqlx::query_as::<_, NewsPlaylist>(
        &format!(
            r"UPDATE news_playlists SET
               name = COALESCE($1, name),
               description = COALESCE($2, description),
               updated_at = $3
           WHERE id = $4
           RETURNING {}",
            PLAYLIST_COLUMNS
        ),
    )
    .bind(name)
    .bind(description)
    .bind(now)
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn destroy(pool: &sqlx::PgPool, id: i64) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM news_playlists WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    Ok(result.rows_affected() > 0)
}
