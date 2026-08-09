use sqlx::Transaction;

use crate::models::sign_up_trigger::SignUpTrigger;

const TRIGGER_COLUMNS: &str = "id, email, token, expires_at, role, created_at, updated_at";

pub async fn check_artist_conflict(pool: &sqlx::PgPool, email: &str) -> Result<bool, sqlx::Error> {
    let is_artist: bool = sqlx::query_scalar(
        r"SELECT EXISTS(
            SELECT 1 FROM users u
            JOIN artists a ON a.user_id = u.id
            WHERE u.email = $1 AND u.role = 'artist'
        )",
    )
    .bind(email)
    .fetch_one(pool)
    .await
    .unwrap_or(false);

    Ok(is_artist)
}

pub async fn expire_existing(
    pool: &sqlx::PgPool,
    email: &str,
    now_str: &str,
    now: chrono::NaiveDateTime,
) -> Result<(), sqlx::Error> {
    let _ = sqlx::query(
        r"UPDATE sign_up_triggers SET expires_at = $1, updated_at = $2 WHERE email = $3",
    )
    .bind(now_str)
    .bind(now)
    .bind(email)
    .execute(pool)
    .await;

    Ok(())
}

pub async fn create(
    pool: &sqlx::PgPool,
    email: &str,
    token: &str,
    expires_at: &str,
    role: &Option<String>,
    now: chrono::NaiveDateTime,
) -> Result<SignUpTrigger, sqlx::Error> {
    sqlx::query_as::<_, SignUpTrigger>(
        &format!(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING {}",
            TRIGGER_COLUMNS
        ),
    )
    .bind(email)
    .bind(token)
    .bind(expires_at)
    .bind(role)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn by_token(pool: &sqlx::PgPool, token: &str) -> Result<Option<SignUpTrigger>, sqlx::Error> {
    sqlx::query_as::<_, SignUpTrigger>(
        &format!("SELECT {} FROM sign_up_triggers WHERE token = $1", TRIGGER_COLUMNS),
    )
    .bind(token)
    .fetch_optional(pool)
    .await
}

pub async fn by_token_for_update(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    token: &str,
) -> Result<Option<(Option<String>, Option<String>)>, sqlx::Error> {
    let row = sqlx::query_as::<_, (Option<String>, Option<String>)>(
        r"SELECT email, expires_at FROM sign_up_triggers WHERE token = $1 FOR UPDATE",
    )
    .bind(token)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(row)
}
