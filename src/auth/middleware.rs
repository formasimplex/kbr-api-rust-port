use actix_web::{dev::Payload, http::header::AUTHORIZATION, HttpRequest, Error, FromRequest, HttpResponse};

use crate::auth::jwt::{Claims, decode_token};
use crate::auth::roles::Role;

/// Represents the authenticated user extracted from a JWT.
///
/// This is a simple data struct. All database operations related to
/// session management are delegated to [`crate::services::session_service::SessionService`].
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
                if crate::services::session_service::SessionService::is_token_revoked(p, &user.jti).await.unwrap_or(false) {
                    return Err(actix_web::error::ErrorUnauthorized("token has been revoked"));
                }
                if !crate::services::session_service::SessionService::is_token_version_valid(p, user.id, user.token_version).await.unwrap_or(false) {
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
    use sqlx::PgPool;

    async fn get_pool() -> PgPool {
        let pool = sqlx::pool::PoolOptions::new()
            .max_connections(1)
            .connect(&crate::test_utils::test_db_url())
            .await
            .expect("Failed to connect to test database");
        let _ = crate::db::migrate::run_migrations(&pool).await;
        pool
    }

    #[tokio::test(flavor = "current_thread")]
    async fn purge_expired_revoked_tokens_removes_old_entries() {
        let pool = get_pool().await;
        let _guard = crate::test_utils::TestGuard::new(&pool).await;

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

        let purged = crate::services::session_service::SessionService::purge_expired_revoked_tokens(&pool, 7).await.expect("purge should succeed");
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

        _guard.cleanup().await;
    }
}
