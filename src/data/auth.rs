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

#[derive(Debug, FromRow)]
pub struct UserRowNoPassword {
    pub id: i64,
    pub email: String,
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

impl From<UserRowNoPassword> for User {
    fn from(row: UserRowNoPassword) -> Self {
        User {
            id: row.id,
            email: row.email,
            password_digest: String::new(),
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

const USER_COLUMNS: &str =
    r#"id, email, password_digest, role, session_token, username, COALESCE(token_version, 1) as token_version, created_at, updated_at"#;

const USER_COLUMNS_NO_PASSWORD: &str =
    r#"id, email, role, session_token, username, COALESCE(token_version, 1) as token_version, created_at, updated_at"#;

pub async fn find_by_email(pool: &sqlx::PgPool, email: &str) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRow>(
        &format!("SELECT {} FROM users WHERE email = $1", USER_COLUMNS),
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn find_by_id_no_password(pool: &sqlx::PgPool, id: i64) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRowNoPassword>(
        &format!("SELECT {} FROM users WHERE id = $1", USER_COLUMNS_NO_PASSWORD),
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}
