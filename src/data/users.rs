use sqlx::{PgPool, Transaction};

use crate::models::user::{User, UserRow, USER_COLUMNS};

pub async fn list(pool: &PgPool) -> Result<Vec<User>, sqlx::Error> {
    let rows = sqlx::query_as::<_, UserRow>(
        &format!(r"SELECT {} FROM users ORDER BY id", USER_COLUMNS),
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        &format!(r"SELECT {} FROM users WHERE id = $1", USER_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn find_by_email(pool: &sqlx::PgPool, email: &str) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        &format!(r"SELECT {} FROM users WHERE email = $1", USER_COLUMNS),
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn create(
    pool: &sqlx::PgPool,
    email: &str,
    password_digest: &str,
    role: &str,
    username: &Option<String>,
    now: chrono::NaiveDateTime,
) -> Result<User, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        &format!(
            r"INSERT INTO users (email, password_digest, role, username, created_at, updated_at)
              VALUES ($1, $2, $3, $4, $5, $6)
              RETURNING {}",
            USER_COLUMNS
        ),
    )
    .bind(email)
    .bind(password_digest)
    .bind(role)
    .bind(username)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}

pub async fn update(
    pool: &sqlx::PgPool,
    id: i64,
    email: &Option<String>,
    role: &Option<String>,
    username: &Option<String>,
    now: chrono::NaiveDateTime,
) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        &format!(
            r"UPDATE users SET
               email = COALESCE($1, email),
               role = COALESCE($2, role),
               username = COALESCE($3, username),
               updated_at = $4
           WHERE id = $5
           RETURNING {}",
            USER_COLUMNS
        ),
    )
    .bind(email)
    .bind(role)
    .bind(username)
    .bind(now)
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn update_password(
    pool: &sqlx::PgPool,
    id: i64,
    password_digest: &str,
    now: chrono::NaiveDateTime,
) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        &format!(
            r"UPDATE users SET
               password_digest = $1,
               updated_at = $2
           WHERE id = $3
           RETURNING {}",
            USER_COLUMNS
        ),
    )
    .bind(password_digest)
    .bind(now)
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn exists_by_email(pool: &PgPool, email: &str) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
        .bind(email)
        .fetch_one(pool)
        .await?;
    Ok(exists)
}

pub async fn exists_by_email_in_tx(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    email: &str,
) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
        .bind(email)
        .fetch_one(&mut **tx)
        .await?;
    Ok(exists)
}

pub async fn create_in_tx(
    tx: &mut Transaction<'_, sqlx::Postgres>,
    email: &str,
    password_digest: &str,
    role: &str,
    username: &Option<String>,
    now: chrono::NaiveDateTime,
) -> Result<User, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        &format!(
            r"INSERT INTO users (email, password_digest, role, session_token, username, token_version, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, 1, $6, $7)
               RETURNING {}",
            USER_COLUMNS
        ),
    )
    .bind(email)
    .bind(password_digest)
    .bind(role)
    .bind::<Option<String>>(None)
    .bind(username)
    .bind(now)
    .bind(now)
    .fetch_one(&mut **tx)
    .await?;

    Ok(row.into())
}

pub async fn update_with_password(
    pool: &PgPool,
    id: i64,
    email: &Option<String>,
    username: &Option<String>,
    role: &Option<String>,
    password_digest: &Option<String>,
    increment_token_version: bool,
    now: chrono::NaiveDateTime,
) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        &format!(
            r"UPDATE users
               SET email = COALESCE($1, email),
                   username = COALESCE($2, username),
                   role = COALESCE($3, role),
                   password_digest = COALESCE($4, password_digest),
                   token_version = CASE WHEN $7 THEN COALESCE(token_version, 1) + 1 ELSE token_version END,
                   updated_at = $5
               WHERE id = $6
               RETURNING {}",
            USER_COLUMNS
        ),
    )
    .bind(email)
    .bind(username)
    .bind(role)
    .bind(password_digest)
    .bind(now)
    .bind(id)
    .bind(increment_token_version)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}
