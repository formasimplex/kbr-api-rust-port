use actix_web::{Error, FromRequest, HttpResponse};

use crate::auth::jwt::{decode_token, Claims};
use crate::auth::roles::Role;
use crate::error::AppError;

pub const JWT_SECRET_ENV: &str = "JWT_SECRET";

pub fn get_jwt_secret() -> Result<String, AppError> {
    std::env::var(JWT_SECRET_ENV).map_err(|_| {
        AppError::Internal("JWT_SECRET not configured".to_string())
    })
}

#[derive(Debug, Clone)]
pub struct CurrentUser {
    pub id: i64,
    pub role: Role,
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
    type Future = std::pin::Pin<Box<dyn std::future::Future<Output = Result<Self, Self::Error>> + 'static>>;

    fn from_request(req: &actix_web::HttpRequest, _payload: &mut actix_web::dev::Payload) -> Self::Future {
        let header = req.headers().get(actix_web::http::header::AUTHORIZATION).cloned();
        let secret = match get_jwt_secret() {
            Ok(s) => s,
            Err(_) => {
                return Box::pin(async move {
                    Err(actix_web::error::ErrorInternalServerError(
                        "JWT_SECRET not configured",
                    ))
                });
            }
        };

        Box::pin(async move {
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

            let claims: Claims = decode_token(token, &secret)
                .map_err(|_| actix_web::error::ErrorUnauthorized("invalid or expired token"))?;

            Ok(CurrentUser {
                id: claims.user_id,
                role: claims
                    .role
                    .and_then(|r| r.parse::<Role>().ok())
                    .unwrap_or(Role::User),
            })
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
    use crate::auth::jwt::encode_token;
    use actix_web::{test, web, App};

    const TEST_SECRET: &str = "test-secret-key";

    #[tokio::test(flavor = "current_thread")]
    async fn protected_endpoint_rejects_missing_token() {
        unsafe { std::env::set_var(JWT_SECRET_ENV, TEST_SECRET); }

        let app = test::init_service(
            App::new().route("/protected", web::get().to(protected_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/protected")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn protected_endpoint_rejects_invalid_token() {
        unsafe { std::env::set_var(JWT_SECRET_ENV, TEST_SECRET); }

        let app = test::init_service(
            App::new().route("/protected", web::get().to(protected_handler)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header(("Authorization", "Bearer invalid.token.here"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn protected_endpoint_accepts_valid_token() {
        unsafe { std::env::set_var(JWT_SECRET_ENV, TEST_SECRET); }

        let app = test::init_service(
            App::new().route("/protected", web::get().to(protected_handler)),
        )
        .await;

        let token = encode_token(42, TEST_SECRET, 3).expect("should encode");
        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn protected_endpoint_rejects_expired_token() {
        unsafe { std::env::set_var(JWT_SECRET_ENV, TEST_SECRET); }

        let app = test::init_service(
            App::new().route("/protected", web::get().to(protected_handler)),
        )
        .await;

        let token = crate::auth::jwt::create_expired_token(42, TEST_SECRET);

        let req = test::TestRequest::get()
            .uri("/protected")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 401);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn current_user_is_admin() {
        let user = CurrentUser {
            id: 1,
            role: Role::Admin,
        };
        assert!(user.is_admin());
        assert!(user.is_artist_or_above());

        let regular = CurrentUser {
            id: 2,
            role: Role::User,
        };
        assert!(!regular.is_admin());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn get_jwt_secret_errors_when_not_set() {
        unsafe { std::env::remove_var(JWT_SECRET_ENV); }
        let result = get_jwt_secret();
        assert!(result.is_err(), "Should error when JWT_SECRET is not set");
        unsafe { std::env::set_var(JWT_SECRET_ENV, TEST_SECRET); }
    }

 }
