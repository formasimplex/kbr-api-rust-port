use crate::models::mail_subscriber::MailSubscriber;

const SUBSCRIBER_COLUMNS: &str =
    r#"id, full_name, email, active, artist_id, unsubscribed_at,
        unsubscribe_token, unsubscribe_token_expires_at, user_id, created_at, updated_at"#;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<MailSubscriber>, sqlx::Error> {
    sqlx::query_as::<_, MailSubscriber>(
        &format!(r"SELECT {} FROM mail_subscribers ORDER BY id", SUBSCRIBER_COLUMNS),
    )
    .fetch_all(pool)
    .await
}

pub async fn list_by_artist(pool: &sqlx::PgPool, artist_id: i64) -> Result<Vec<MailSubscriber>, sqlx::Error> {
    sqlx::query_as::<_, MailSubscriber>(
        &format!(r"SELECT {} FROM mail_subscribers WHERE artist_id = $1 ORDER BY id", SUBSCRIBER_COLUMNS),
    )
    .bind(artist_id)
    .fetch_all(pool)
    .await
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<MailSubscriber>, sqlx::Error> {
    sqlx::query_as::<_, MailSubscriber>(
        &format!(r"SELECT {} FROM mail_subscribers WHERE id = $1", SUBSCRIBER_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_email_and_artist(
    pool: &sqlx::PgPool,
    email: &str,
    artist_id: i64,
) -> Result<Option<MailSubscriber>, sqlx::Error> {
    sqlx::query_as::<_, MailSubscriber>(
        &format!(r"SELECT {} FROM mail_subscribers WHERE email = $1 AND artist_id = $2", SUBSCRIBER_COLUMNS),
    )
    .bind(email)
    .bind(artist_id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_email(pool: &sqlx::PgPool, email: &str) -> Result<Option<MailSubscriber>, sqlx::Error> {
    sqlx::query_as::<_, MailSubscriber>(
        &format!(r"SELECT {} FROM mail_subscribers WHERE email = $1", SUBSCRIBER_COLUMNS),
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_token(pool: &sqlx::PgPool, token: &str) -> Result<Option<MailSubscriber>, sqlx::Error> {
    sqlx::query_as::<_, MailSubscriber>(
        &format!(r"SELECT {} FROM mail_subscribers WHERE unsubscribe_token = $1", SUBSCRIBER_COLUMNS),
    )
    .bind(token)
    .fetch_optional(pool)
    .await
}

pub async fn create(
    pool: &sqlx::PgPool,
    full_name: &str,
    email: &str,
    artist_id: Option<i64>,
    user_id: Option<i64>,
    now: chrono::NaiveDateTime,
) -> Result<MailSubscriber, sqlx::Error> {
    sqlx::query_as::<_, MailSubscriber>(
        &format!(
            r"INSERT INTO mail_subscribers (full_name, email, artist_id, user_id, created_at, updated_at)
              VALUES ($1, $2, $3, $4, $5, $6)
              RETURNING {}",
            SUBSCRIBER_COLUMNS
        ),
    )
    .bind(full_name)
    .bind(email)
    .bind(artist_id)
    .bind(user_id)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn set_unsubscribe_token(
    pool: &sqlx::PgPool,
    email: &str,
    token: &str,
    expires_at: chrono::NaiveDateTime,
    now: chrono::NaiveDateTime,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r"UPDATE mail_subscribers SET unsubscribe_token = $1, unsubscribe_token_expires_at = $2, updated_at = $3 WHERE email = $4"
    )
    .bind(token)
    .bind(expires_at)
    .bind(now)
    .bind(email)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn unsubscribe(pool: &sqlx::PgPool, user_id: i64, artist_id: i64, now: chrono::NaiveDateTime) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r"UPDATE mail_subscribers SET unsubscribed_at = $1, updated_at = $2 WHERE user_id = $3 AND artist_id = $4 AND unsubscribed_at IS NULL"
    )
    .bind(now)
    .bind(now)
    .bind(user_id)
    .bind(artist_id)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn process_unsubscribe(pool: &sqlx::PgPool, token: &str, now: chrono::NaiveDateTime) -> Result<Option<String>, sqlx::Error> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"UPDATE mail_subscribers SET unsubscribed_at = $1, updated_at = $2, unsubscribe_token = NULL
           WHERE unsubscribe_token = $3 AND unsubscribed_at IS NULL
             AND (unsubscribe_token_expires_at IS NULL OR unsubscribe_token_expires_at > $1)
           RETURNING email"#,
    )
    .bind(now)
    .bind(now)
    .bind(token)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.0))
}
