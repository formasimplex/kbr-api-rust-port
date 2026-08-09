use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::middleware::CurrentUser;
use crate::auth::roles::Role;
use crate::error::AppError;

/// Service that handles session/token lifecycle operations against the database.
///
/// This isolates SQL queries from the auth middleware layer, allowing
/// `CurrentUser` to remain a simple data struct and enabling testing
/// without a live database.
pub struct SessionService;

impl SessionService {
    /// Verify the user's current role against the database.
    ///
    /// Returns the role stored in the `users` table, which may differ
    /// from the role embedded in the JWT if it was changed after issuance.
    pub async fn verify_role_with_db(pool: &PgPool, user_id: i64) -> Result<Role, AppError> {
        let db_role: String = sqlx::query_scalar(
            "SELECT role FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::Unauthorized)?;
        db_role.parse::<Role>().map_err(|_| AppError::Unauthorized)
    }

    /// Check whether the given token's JTI has been revoked.
    pub async fn is_token_revoked(pool: &PgPool, jti: &str) -> Result<bool, AppError> {
        if jti.is_empty() {
            return Ok(false);
        }
        let jti_uuid = match Uuid::parse_str(jti) {
            Ok(u) => u,
            Err(_) => return Ok(false),
        };
        let is_revoked: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM revoked_tokens WHERE jti = $1)"
        )
        .bind(jti_uuid)
        .fetch_optional(pool)
        .await
        .unwrap_or(Some(false))
        .unwrap_or(false);
        Ok(is_revoked)
    }

    /// Check whether the token's version is still valid.
    ///
    /// Returns `true` if the token's version is >= the user's current
    /// `token_version` in the database.
    pub async fn is_token_version_valid(
        pool: &PgPool,
        user_id: i64,
        token_version: i64,
    ) -> Result<bool, AppError> {
        let db_version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(token_version, 1) FROM users WHERE id = $1"
        )
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap_or(1);
        Ok(token_version >= db_version)
    }

    /// Revoke a single token by inserting its JTI into `revoked_tokens`.
    pub async fn revoke_token(pool: &PgPool, jti: &str) -> Result<(), AppError> {
        if jti.is_empty() {
            return Ok(());
        }
        let jti_uuid = Uuid::parse_str(jti).map_err(|_| AppError::Internal("invalid jti format".to_string()))?;
        let _ = sqlx::query(
            "INSERT INTO revoked_tokens (jti) VALUES ($1) ON CONFLICT (jti) DO NOTHING"
        )
        .bind(jti_uuid)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, jti = %jti, "Failed to revoke token");
            AppError::Internal("Failed to revoke token".to_string())
        })?;
        Ok(())
    }

    /// Invalidate all sessions for a user by incrementing `token_version`.
    pub async fn invalidate_all_sessions(pool: &PgPool, user_id: i64) -> Result<(), AppError> {
        let _ = sqlx::query(
            "UPDATE users SET token_version = COALESCE(token_version, 1) + 1, updated_at = NOW() WHERE id = $1"
        )
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = user_id, "Failed to invalidate sessions");
            AppError::Internal("Failed to invalidate sessions".to_string())
        })?;
        Ok(())
    }

    /// Convenience wrapper that performs the full logout flow:
    /// revoke the current token and invalidate all sessions.
    pub async fn logout(pool: &PgPool, user: &CurrentUser) -> Result<(), AppError> {
        Self::revoke_token(pool, &user.jti).await?;
        Self::invalidate_all_sessions(pool, user.id).await?;
        Ok(())
    }

    /// Convenience wrapper that validates a token during middleware extraction:
    /// checks revocation and version in a single call.
    pub async fn validate_token(pool: &PgPool, user: &CurrentUser) -> Result<(), AppError> {
        if Self::is_token_revoked(pool, &user.jti).await.unwrap_or(false) {
            return Err(AppError::Unauthorized);
        }
        if !Self::is_token_version_valid(pool, user.id, user.token_version).await.unwrap_or(false) {
            return Err(AppError::Unauthorized);
        }
        Ok(())
    }

    /// Purge revoked tokens older than the given retention period.
    ///
    /// Call periodically (e.g., from a background job) to prevent database bloat.
    /// Default retention of 7 days covers the 3-day JWT expiry plus a safety buffer.
    pub async fn purge_expired_revoked_tokens(
        pool: &PgPool,
        retention_days: i64,
    ) -> Result<u64, AppError> {
        let result = sqlx::query(
            "DELETE FROM revoked_tokens WHERE revoked_at < NOW() - INTERVAL '$1 days'"
        )
        .bind(retention_days.to_string())
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to purge expired revoked tokens");
            AppError::Internal("Failed to purge expired revoked tokens".to_string())
        })?;
        Ok(result.rows_affected())
    }
}
