use sqlx::PgPool;

use crate::error::AppError;

pub async fn connect() -> Result<PgPool, AppError> {
    let database_url = std::env::var("DATABASE_URL").map_err(|e| {
        AppError::Internal(format!("DATABASE_URL not set: {}", e))
    })?;

    PgPool::connect(&database_url).await.map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connect_fails_when_database_url_not_set() {
        // In Rust 2024, env::remove_var is unsafe when other threads may be running
        // Temporarily set an invalid value to simulate missing DATABASE_URL
        let original = std::env::var("DATABASE_URL").ok();
        unsafe {
            std::env::set_var("DATABASE_URL", "");
        }

        let result = connect().await;

        if let Some(val) = original {
            unsafe {
                std::env::set_var("DATABASE_URL", val);
            }
        }

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn connect_fails_with_invalid_url() {
        unsafe {
            std::env::set_var("DATABASE_URL", "postgresql://invalid-host-xyz123:5432/nonexistent");
        }
        let result = connect().await;
        assert!(result.is_err());
        matches!(result.unwrap_err(), AppError::Database(_));
    }
}
