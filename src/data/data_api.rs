use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, FromRow)]
pub struct LastLoginRow {
    pub username: Option<String>,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, FromRow)]
pub struct LastLoginByIdRow {
    pub email: String,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug, FromRow, Serialize)]
pub struct EventAttendeePresentRow {
    pub id: i64,
    pub scan_count: Option<i32>,
    pub email: String,
    pub full_name: String,
}

pub async fn last_logins(pool: &sqlx::PgPool) -> Result<Vec<LastLoginRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, LastLoginRow>(
        r#"SELECT username, updated_at FROM users ORDER BY updated_at DESC LIMIT 10"#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn last_login_by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<LastLoginByIdRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, LastLoginByIdRow>(
        r#"SELECT email, updated_at FROM users WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn event_exists(pool: &sqlx::PgPool, event_id: i64) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM kbr_events WHERE id = $1"#,
    )
    .bind(event_id)
    .fetch_one(pool)
    .await?;

    Ok(count > 0)
}

pub async fn event_attendees_present(pool: &sqlx::PgPool, event_id: i32) -> Result<Vec<EventAttendeePresentRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, EventAttendeePresentRow>(
        r#"
        SELECT DISTINCT ON (ma.mail_subscriber_id)
            ma.id,
            ma.scan_count,
            ms.email,
            ms.full_name
        FROM kbr_event_attendees ma
        JOIN mail_subscribers ms ON ms.id = ma.mail_subscriber_id
        WHERE ma.kbr_event_id = $1 AND ma.scan_count > 0
        ORDER BY ma.mail_subscriber_id, ma.id
        "#,
    )
    .bind(event_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
