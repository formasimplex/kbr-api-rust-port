use sqlx::PgPool;

use crate::error::AppError;

pub async fn connect() -> Result<PgPool, AppError> {
    let database_url = std::env::var("DATABASE_URL").map_err(|e| {
        AppError::Internal(format!("DATABASE_URL not set: {}", e))
    })?;

    PgPool::connect(&database_url).await.map_err(Into::into)
}


