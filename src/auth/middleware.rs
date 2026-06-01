use actix_web::{dev::Payload, http::header::AUTHORIZATION, HttpRequest, Error, FromRequest, HttpResponse};
use sqlx::PgPool;

use crate::auth::jwt::{Claims, decode_token};
use crate::auth::roles::Role;
use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: i64,
    pub role: Role,
    pub jti: String,
    pub token_version: i64,
}

impl CurrentUser {
    pub fn is_admin(&self) -> bool {
        crate::auth::roles::is_admin(&self.role)
    }

    pub fn is_artist_or_above(&self) -> bool {
        crate::auth::roles::is_artist_or_above(&self.role)
    }

    pub async fn verify_role_with_db(&self, pool: &PgPool) -> Result<Role, AppError> {
        let db_role: String = sqlx::query_scalar(
            "SELECT role FROM users WHERE id = $1"
        )
        .bind(self.id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::Unauthorized)?;
        db_role.parse::<Role>().map_err(|_| AppError::Unauthorized)
    }

    pub async fn check_token_revoked(&self, pool: &PgPool) -> Result<bool, AppError> {
        if self.jti.is_empty() {
            return Ok(false);
        }
        let jti_uuid = match uuid::Uuid::parse_str(&self.jti) {
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

    pub async fn check_token_version(&self, pool: &PgPool) -> Result<bool, AppError> {
        let db_version: i64 = sqlx::query_scalar(
            "SELECT COALESCE(token_version, 1) FROM users WHERE id = $1"
        )
        .bind(self.id)
        .fetch_one(pool)
        .await
        .unwrap_or(1);
        Ok(self.token_version >= db_version)
    }

    pub async fn revoke_token(&self, pool: &PgPool) -> Result<(), AppError> {
        if self.jti.is_empty() {
            return Ok(());
        }
        let jti_uuid = uuid::Uuid::parse_str(&self.jti).map_err(|_| AppError::Internal("invalid jti format".to_string()))?;
        let _ = sqlx::query(
            "INSERT INTO revoked_tokens (jti) VALUES ($1) ON CONFLICT (jti) DO NOTHING"
        )
        .bind(jti_uuid)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, jti = %self.jti, "Failed to revoke token");
            AppError::Internal("Failed to revoke token".to_string())
        })?;
        Ok(())
    }

    pub async fn invalidate_all_sessions(&self, pool: &PgPool) -> Result<(), AppError> {
        let _ = sqlx::query(
            "UPDATE users SET token_version = COALESCE(token_version, 1) + 1, updated_at = NOW() WHERE id = $1"
        )
        .bind(self.id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = self.id, "Failed to invalidate sessions");
            AppError::Internal("Failed to invalidate sessions".to_string())
        })?;
        Ok(())
    }
}

/// Purge revoked tokens older than the given retention period.
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

impl FromRequest for CurrentUser {
    type Error = Error;
    type Future =
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self, Self::Error>> + 'static>>;

    fn from_request(
        req: &HttpRequest,
        _payload: &mut Payload,
    ) -> Self::Future {
        let header = req
            .headers()
            .get(AUTHORIZATION)
            .cloned();

        let cookie_value = req
            .app_data::<actix_web::web::Data<actix_jc::ActixJwtCookie<Claims>>>()
            .and_then(|builder| req.cookie(builder.name.as_ref()).map(|c| c.value().to_string()));

        let secret = match std::env::var("JWT_SECRET") {
            Ok(s) => s,
            Err(_) => {
                return Box::pin(async move {
                    Err(actix_web::error::ErrorInternalServerError(
                        "JWT_SECRET not configured",
                    ))
                });
            }
        };

        let pool = req
            .app_data::<actix_web::web::Data<crate::app::AppState>>()
            .map(|s| s.db.clone());

        Box::pin(async move {
            let claims = if let Some(cookie) = cookie_value {
                match decode_token(&cookie, &secret) {
                    Ok(claims) => claims,
                    Err(_) => {
                        let token = match &header {
                            Some(h) => {
                                let header_str = h.to_str().map_err(|_| {
                                    actix_web::error::ErrorUnauthorized("invalid Authorization header encoding")
                                })?;
                                header_str.strip_prefix("Bearer ").ok_or_else(|| {
                                    actix_web::error::ErrorUnauthorized("missing Bearer prefix")
                                })?
                            }
                            None => {
                                return Err(actix_web::error::ErrorUnauthorized(
                                    "missing Authorization header or valid cookie",
                                ));
                            }
                        };

                        decode_token(token, &secret)
                            .map_err(|_| actix_web::error::ErrorUnauthorized("invalid or expired token"))?
                    }
                }
            } else {
                let token = match &header {
                    Some(h) => {
                        let header_str = h.to_str().map_err(|_| {
                            actix_web::error::ErrorUnauthorized("invalid Authorization header encoding")
                        })?;
                        header_str.strip_prefix("Bearer ").ok_or_else(|| {
                            actix_web::error::ErrorUnauthorized("missing Bearer prefix")
                        })?
                    }
                    None => {
                        return Err(actix_web::error::ErrorUnauthorized(
                            "missing Authorization header",
                        ));
                    }
                };

                decode_token(token, &secret)
                    .map_err(|_| actix_web::error::ErrorUnauthorized("invalid or expired token"))?
            };

            if claims.iat == 0 {
                return Err(actix_web::error::ErrorUnauthorized("token missing issued-at claim"));
            }

            let role = match claims.role.as_ref().and_then(|r| r.parse::<Role>().ok()) {
                Some(r) => r,
                None => {
                    tracing::warn!(
                        user_id = claims.user_id,
                        raw_role = ?claims.role,
                        "Invalid or missing role in JWT, rejecting token"
                    );
                    return Err(actix_web::error::ErrorUnauthorized(
                        "invalid role in token",
                    ));
                }
            };

            let user = CurrentUser {
                id: claims.user_id,
                role,
                jti: claims.jti,
                token_version: claims.token_version,
            };

            if let Some(p) = &pool {
                if user.check_token_revoked(p).await.unwrap_or(false) {
                    return Err(actix_web::error::ErrorUnauthorized("token has been revoked"));
                }
                if !user.check_token_version(p).await.unwrap_or(false) {
                    return Err(actix_web::error::ErrorUnauthorized("token version outdated, please log in again"));
                }
            }

            Ok(user)
        })
    }
}

pub async fn protected_handler(user: CurrentUser) -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "user_id": user.id,
        "role": user.role.to_string()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    const TEST_DB_URL: &str = "postgresql://ws@localhost:5432/kbr_test";

    async fn get_pool() -> PgPool {
        PgPool::connect(TEST_DB_URL)
            .await
            .expect("Failed to connect to test database")
    }

    #[tokio::test(flavor = "current_thread")]
    async fn purge_expired_revoked_tokens_removes_old_entries() {
        let pool = get_pool().await;

        // Clean up any leftover tokens from prior test runs
        let _ = sqlx::query("DELETE FROM revoked_tokens").execute(&pool).await;

        let old_jti = uuid::Uuid::new_v4();
        let new_jti = uuid::Uuid::new_v4();

        sqlx::query(
            "INSERT INTO revoked_tokens (jti, revoked_at) VALUES ($1, NOW() - INTERVAL '10 days')"
        )
        .bind(&old_jti)
        .execute(&pool)
        .await
        .expect("Failed to insert old revoked token");

        sqlx::query(
            "INSERT INTO revoked_tokens (jti, revoked_at) VALUES ($1, NOW())"
        )
        .bind(&new_jti)
        .execute(&pool)
        .await
        .expect("Failed to insert new revoked token");

        let purged = purge_expired_revoked_tokens(&pool, 7).await.expect("purge should succeed");
        assert_eq!(purged, 1, "Should have purged 1 old token");

        let remaining: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM revoked_tokens WHERE jti = $1"
        )
        .bind(&new_jti)
        .fetch_one(&pool)
        .await
        .expect("Failed to count remaining tokens");
        assert_eq!(remaining, 1, "New token should still exist");

        let gone: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM revoked_tokens WHERE jti = $1"
        )
        .bind(&old_jti)
        .fetch_one(&pool)
        .await
        .expect("Failed to count purged tokens");
        assert_eq!(gone, 0, "Old token should be purged");
    }
}
