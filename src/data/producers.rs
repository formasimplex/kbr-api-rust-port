use sqlx::FromRow;

use crate::models::producer::Producer;

#[derive(Debug, FromRow)]
pub struct ProducerRow {
    pub id: i64,
    pub description: Option<String>,
    pub producer_name: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<ProducerRow> for Producer {
    fn from(row: ProducerRow) -> Self {
        Producer {
            id: row.id,
            description: row.description,
            producer_name: row.producer_name,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Producer>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ProducerRow>(
        r"SELECT id, description, producer_name, created_at, updated_at FROM producers ORDER BY id"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn create(
    pool: &sqlx::PgPool,
    producer_name: &str,
    description: Option<&str>,
    now: chrono::NaiveDateTime,
) -> Result<Producer, sqlx::Error> {
    let row = sqlx::query_as::<_, ProducerRow>(
        r"INSERT INTO producers (producer_name, description, created_at, updated_at)
           VALUES ($1, $2, $3, $4)
           RETURNING id, description, producer_name, created_at, updated_at"
    )
    .bind(producer_name)
    .bind(description)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}

pub async fn update(
    pool: &sqlx::PgPool,
    id: i64,
    producer_name: Option<&str>,
    description: Option<&str>,
    now: chrono::NaiveDateTime,
) -> Result<Option<Producer>, sqlx::Error> {
    let row = sqlx::query_as::<_, ProducerRow>(
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
    .await?;

    Ok(row.map(|r| r.into()))
}
