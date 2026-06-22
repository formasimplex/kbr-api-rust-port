use crate::models::producer::Producer;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Producer>, sqlx::Error> {
    sqlx::query_as::<_, Producer>(
        r"SELECT id, description, producer_name, created_at, updated_at FROM producers ORDER BY id"
    )
    .fetch_all(pool)
    .await
}

pub async fn create(
    pool: &sqlx::PgPool,
    producer_name: &str,
    description: Option<&str>,
    now: chrono::NaiveDateTime,
) -> Result<Producer, sqlx::Error> {
    sqlx::query_as::<_, Producer>(
        r"INSERT INTO producers (producer_name, description, created_at, updated_at)
           VALUES ($1, $2, $3, $4)
           RETURNING id, description, producer_name, created_at, updated_at"
    )
    .bind(producer_name)
    .bind(description)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn update(
    pool: &sqlx::PgPool,
    id: i64,
    producer_name: Option<&str>,
    description: Option<&str>,
    now: chrono::NaiveDateTime,
) -> Result<Option<Producer>, sqlx::Error> {
    sqlx::query_as::<_, Producer>(
        r"UPDATE producers
           SET producer_name = COALESCE($1, producer_name),
               description = COALESCE($2, description),
               updated_at = $3
           WHERE id = $4
           RETURNING id, description, producer_name, created_at, updated_at"
    )
    .bind(producer_name)
    .bind(description)
    .bind(now)
    .bind(id)
    .fetch_optional(pool)
    .await
}
