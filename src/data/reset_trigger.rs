use sqlx::FromRow;

use crate::models::reset_trigger::ResetTrigger;

#[derive(Debug, FromRow)]
pub struct ResetTriggerRow {
    pub id: i64,
    pub user_id: Option<i32>,
    pub token: Option<String>,
    pub expires_at: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<ResetTriggerRow> for ResetTrigger {
    fn from(row: ResetTriggerRow) -> Self {
        ResetTrigger {
            id: row.id,
            email: None,
            user_id: row.user_id,
            full_name: None,
            token: row.token,
            expires_at: row.expires_at,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const TRIGGER_COLUMNS: &str = "id, user_id, token, expires_at, created_at, updated_at";

pub async fn find_user_by_email(pool: &sqlx::PgPool, email: &str) -> Result<Option<i64>, sqlx::Error> {
    let id: Option<i64> = sqlx::query_scalar(
        r"SELECT id FROM users WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(id)
}

pub async fn create(
    pool: &sqlx::PgPool,
    user_id: Option<i32>,
    token: &str,
    expires_at: &str,
    now: chrono::NaiveDateTime,
) -> Result<ResetTrigger, sqlx::Error> {
    let row = sqlx::query_as::<_, ResetTriggerRow>(
        &format!(
            r"INSERT INTO reset_triggers (user_id, token, expires_at, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5)
               RETURNING {}",
            TRIGGER_COLUMNS
        ),
    )
    .bind(user_id)
    .bind(token)
    .bind(expires_at)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}

pub async fn find_by_token_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token: &str,
) -> Result<Option<ResetTriggerRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, ResetTriggerRow>(
        &format!(
            r"SELECT {} FROM reset_triggers WHERE token = $1 FOR UPDATE",
            TRIGGER_COLUMNS
        ),
    )
    .bind(token)
    .fetch_optional(&mut **tx)
    .await?;

    Ok(row)
}

pub async fn get_current_password(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
) -> Result<String, sqlx::Error> {
    let hash: String = sqlx::query_scalar(
        "SELECT password_digest FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(&mut **tx)
    .await?;

    Ok(hash)
}

pub async fn update_password_and_invalidate(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: i64,
    new_hash: &str,
    now: chrono::NaiveDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"UPDATE users SET password_digest = $1, token_version = COALESCE(token_version, 1) + 1, updated_at = $2 WHERE id = $3",
    )
    .bind(new_hash)
    .bind(now)
    .bind(user_id)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

pub async fn delete_trigger(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(r"DELETE FROM reset_triggers WHERE token = $1")
        .bind(token)
        .execute(&mut **tx)
        .await?;

    Ok(())
}
