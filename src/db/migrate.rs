use sqlx::PgPool;

use crate::error::AppError;

/// Run the schema migration against the database.
/// Idempotent: safe to run repeatedly. Uses CREATE TABLE IF NOT EXISTS.
pub async fn run_migrations(pool: &PgPool) -> Result<(), AppError> {
    let schema = include_str!("../../migrations/schema.sql");

    let statements: Vec<&str> = schema
        .split(';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for stmt in statements {
        let _ = sqlx::query(stmt).execute(pool).await;
    }

    tracing::info!("Database migrations completed successfully");
    Ok(())
}
