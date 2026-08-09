use crate::models::news::News;

const NEWS_COLUMNS: &str =
    "id, url, title, vote_score, flagged, flagged_at, user_id, image_url, active, comments_enabled, created_at, updated_at";

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<News>, sqlx::Error> {
    sqlx::query_as::<_, News>(
        &format!(r"SELECT {} FROM news ORDER BY id", NEWS_COLUMNS),
    )
    .fetch_all(pool)
    .await
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<News>, sqlx::Error> {
    sqlx::query_as::<_, News>(
        &format!(r"SELECT {} FROM news WHERE id = $1", NEWS_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn exists_by_url(pool: &sqlx::PgPool, url: &str) -> Result<Option<News>, sqlx::Error> {
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM news WHERE url = $1")
        .bind(url)
        .fetch_optional(pool)
        .await?;

    match existing {
        Some((existing_id,)) => {
            let row = sqlx::query_as::<_, News>(
                &format!(r"SELECT {} FROM news WHERE id = $1", NEWS_COLUMNS),
            )
            .bind(existing_id)
            .fetch_one(pool)
            .await?;
            Ok(Some(row))
        }
        None => Ok(None),
    }
}

pub async fn create(
    pool: &sqlx::PgPool,
    url: &str,
    title: &Option<String>,
    user_id: i64,
    image_url: &Option<String>,
    active: bool,
    comments_enabled: bool,
    now: chrono::NaiveDateTime,
) -> Result<News, sqlx::Error> {
    sqlx::query_as::<_, News>(
        &format!(
            r"INSERT INTO news (url, title, user_id, image_url, active, comments_enabled, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             RETURNING {}",
            NEWS_COLUMNS
        ),
    )
    .bind(url)
    .bind(title)
    .bind(user_id)
    .bind(image_url)
    .bind(active)
    .bind(comments_enabled)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &sqlx::PgPool,
    id: i64,
    active: Option<bool>,
    comments_enabled: Option<bool>,
    now: chrono::NaiveDateTime,
) -> Result<Option<News>, sqlx::Error> {
    let row = if let Some(a) = active {
        if let Some(c) = comments_enabled {
            sqlx::query_as::<_, News>(
                &format!(
                    r"UPDATE news SET active = $1, comments_enabled = $2, updated_at = $3 WHERE id = $4 RETURNING {}",
                    NEWS_COLUMNS
                ),
            )
            .bind(a)
            .bind(c)
            .bind(now)
            .bind(id)
            .fetch_one(pool)
            .await?
        } else {
            sqlx::query_as::<_, News>(
                &format!(
                    r"UPDATE news SET active = $1, updated_at = $2 WHERE id = $3 RETURNING {}",
                    NEWS_COLUMNS
                ),
            )
            .bind(a)
            .bind(now)
            .bind(id)
            .fetch_one(pool)
            .await?
        }
    } else if let Some(c) = comments_enabled {
        sqlx::query_as::<_, News>(
            &format!(
                r"UPDATE news SET comments_enabled = $1, updated_at = $2 WHERE id = $3 RETURNING {}",
                NEWS_COLUMNS
            ),
        )
        .bind(c)
        .bind(now)
        .bind(id)
        .fetch_one(pool)
        .await?
    } else {
        return Ok(None);
    };

    Ok(Some(row))
}

pub async fn toggle_comments(
    pool: &sqlx::PgPool,
    id: i64,
    new_value: bool,
    now: chrono::NaiveDateTime,
) -> Result<News, sqlx::Error> {
    sqlx::query_as::<_, News>(
        &format!(
            r"UPDATE news SET comments_enabled = $1, updated_at = $2 WHERE id = $3 RETURNING {}",
            NEWS_COLUMNS
        ),
    )
    .bind(new_value)
    .bind(now)
    .bind(id)
    .fetch_one(pool)
    .await
}

pub async fn news_exists(pool: &sqlx::PgPool, news_id: i64) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM news WHERE id = $1)")
        .bind(news_id)
        .fetch_one(pool)
        .await?;
    Ok(exists)
}

pub async fn get_playlist(pool: &sqlx::PgPool, playlist_id: i64) -> Result<Option<(i64, i64)>, sqlx::Error> {
    let playlist: Option<(i64, i64)> = sqlx::query_as("SELECT id, user_id FROM news_playlists WHERE id = $1")
        .bind(playlist_id)
        .fetch_optional(pool)
        .await?;
    Ok(playlist)
}

pub async fn playlist_has_news(pool: &sqlx::PgPool, playlist_id: i64, news_id: i64) -> Result<bool, sqlx::Error> {
    let existing: Option<(i64,)> = sqlx::query_as("SELECT id FROM users_news WHERE playlist_id = $1 AND news_id = $2")
        .bind(playlist_id)
        .bind(news_id)
        .fetch_optional(pool)
        .await?;
    Ok(existing.is_some())
}

pub async fn max_position_in_playlist(pool: &sqlx::PgPool, playlist_id: i64) -> Result<Option<i32>, sqlx::Error> {
    let max_position: Option<i32> = sqlx::query_scalar(
        r"SELECT COALESCE(MAX(position), -1) FROM users_news WHERE playlist_id = $1",
    )
    .bind(playlist_id)
    .fetch_optional(pool)
    .await?;
    Ok(max_position)
}

pub async fn add_news_to_playlist(
    pool: &sqlx::PgPool,
    user_id: i64,
    news_id: i64,
    playlist_id: i64,
    position: i32,
    now: chrono::NaiveDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"INSERT INTO users_news (user_id, news_id, playlist_id, position, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(user_id)
    .bind(news_id)
    .bind(playlist_id)
    .bind(position)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}
