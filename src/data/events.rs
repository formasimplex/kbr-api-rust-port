use crate::models::kbr_event::KbrEvent;

const EVENT_COLUMNS: &str =
    "id, name, description, active, event_start_date, event_end_date, create_by_user_id, event_url, qr_encode_string, ticket_url, external_url, created_at, updated_at";

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<KbrEvent>, sqlx::Error> {
    sqlx::query_as::<_, KbrEvent>(
        &format!(r"SELECT {} FROM kbr_events ORDER BY id", EVENT_COLUMNS),
    )
    .fetch_all(pool)
    .await
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<KbrEvent>, sqlx::Error> {
    sqlx::query_as::<_, KbrEvent>(
        &format!(r"SELECT {} FROM kbr_events WHERE id = $1", EVENT_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_by_user(pool: &sqlx::PgPool, user_id: i64) -> Result<Vec<KbrEvent>, sqlx::Error> {
    sqlx::query_as::<_, KbrEvent>(
        &format!(r"SELECT {} FROM kbr_events WHERE create_by_user_id = $1 ORDER BY id", EVENT_COLUMNS),
    )
    .bind(user_id as i32)
    .fetch_all(pool)
    .await
}

pub async fn create(
    pool: &sqlx::PgPool,
    name: &str,
    description: Option<&str>,
    active: bool,
    event_start_date: Option<chrono::NaiveDateTime>,
    event_end_date: Option<chrono::NaiveDateTime>,
    create_by_user_id: i32,
    event_url: Option<&str>,
    qr_encode_string: Option<&str>,
    ticket_url: Option<&str>,
    external_url: Option<&str>,
    now: chrono::NaiveDateTime,
) -> Result<KbrEvent, sqlx::Error> {
    sqlx::query_as::<_, KbrEvent>(
        &format!(
            r"INSERT INTO kbr_events (name, description, active, event_start_date, event_end_date, create_by_user_id, event_url, qr_encode_string, ticket_url, external_url, created_at, updated_at)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
              RETURNING {}",
            EVENT_COLUMNS
        ),
    )
    .bind(name)
    .bind(description)
    .bind(active)
    .bind(event_start_date)
    .bind(event_end_date)
    .bind(create_by_user_id)
    .bind(event_url)
    .bind(qr_encode_string)
    .bind(ticket_url)
    .bind(external_url)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &sqlx::PgPool,
    id: i64,
    name: Option<&str>,
    description: Option<&str>,
    active: Option<bool>,
    event_start_date: Option<chrono::NaiveDateTime>,
    event_end_date: Option<chrono::NaiveDateTime>,
    event_url: Option<&str>,
    qr_encode_string: Option<&str>,
    ticket_url: Option<&str>,
    external_url: Option<&str>,
    now: chrono::NaiveDateTime,
) -> Result<Option<KbrEvent>, sqlx::Error> {
    sqlx::query_as::<_, KbrEvent>(
        &format!(
            r"UPDATE kbr_events SET
               name = COALESCE($1, name),
               description = COALESCE($2, description),
               active = COALESCE($3, active),
               event_start_date = COALESCE($4, event_start_date),
               event_end_date = COALESCE($5, event_end_date),
               event_url = COALESCE($6, event_url),
               qr_encode_string = COALESCE($7, qr_encode_string),
               ticket_url = COALESCE($8, ticket_url),
               external_url = COALESCE($9, external_url),
               updated_at = $10
           WHERE id = $11
           RETURNING {}",
            EVENT_COLUMNS
        ),
    )
    .bind(name)
    .bind(description)
    .bind(active)
    .bind(event_start_date)
    .bind(event_end_date)
    .bind(event_url)
    .bind(qr_encode_string)
    .bind(ticket_url)
    .bind(external_url)
    .bind(now)
    .bind(id)
    .fetch_optional(pool)
    .await
}
