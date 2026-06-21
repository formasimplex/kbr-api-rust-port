use sqlx::FromRow;

use crate::models::user::User;

#[derive(Debug, FromRow)]
pub struct UserRow {
    pub id: i64,
    pub email: String,
    pub password_digest: String,
    pub role: Option<String>,
    pub session_token: Option<String>,
    pub username: Option<String>,
    pub token_version: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        User {
            id: row.id,
            email: row.email,
            password_digest: row.password_digest,
            role: row.role,
            session_token: row.session_token,
            username: row.username,
            first_name: None,
            last_name: None,
            token_version: row.token_version,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const USER_COLUMNS: &str = "id, email, password_digest, role, session_token, username, COALESCE(token_version, 1) as token_version, created_at, updated_at";

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<User>, sqlx::Error> {
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

pub async fn find_by_email(pool: &sqlx::PgPool, email: &str) -> Result<Option<UserRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        &format!(r"SELECT {} FROM users WHERE email = $1", USER_COLUMNS),
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(row)
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

pub async fn exists_by_email(pool: &sqlx::PgPool, email: &str) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)")
        .bind(email)
        .fetch_one(pool)
        .await?;
    Ok(exists)
}
