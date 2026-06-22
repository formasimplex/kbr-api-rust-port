use crate::models::album::Album;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Album>, sqlx::Error> {
    sqlx::query_as::<_, Album>(
        r"SELECT id, name, release_date, created_at, updated_at FROM albums ORDER BY id",
    )
    .fetch_all(pool)
    .await
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<Album>, sqlx::Error> {
    sqlx::query_as::<_, Album>(
        r"SELECT id, name, release_date, created_at, updated_at FROM albums WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create(
    pool: &sqlx::PgPool,
    name: &str,
    release_date: Option<chrono::NaiveDate>,
    now: chrono::NaiveDateTime,
) -> Result<Album, sqlx::Error> {
    sqlx::query_as::<_, Album>(
        r"INSERT INTO albums (name, release_date, created_at, updated_at)
           VALUES ($1, $2, $3, $4)
           RETURNING id, name, release_date, created_at, updated_at",
    )
    .bind(name)
    .bind(release_date)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}
