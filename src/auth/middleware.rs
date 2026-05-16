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
        .bind(&jti_uuid)
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
        .bind(&jti_uuid)
        .execute(pool)
        .await;
        Ok(())
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

            let user = CurrentUser {
                id: claims.user_id,
                role: claims
                    .role
                    .and_then(|r| r.parse::<Role>().ok())
                    .unwrap_or(Role::User),
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
