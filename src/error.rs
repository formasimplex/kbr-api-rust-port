use actix_web::http::StatusCode;
use actix_web::HttpResponse;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("configuration error: {0}")]
    Config(#[from] dotenvy::Error),

    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("authentication required")]
    Unauthorized,

    #[error("forbidden: {0}")]
    Forbidden(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("unprocessable entity: {0}")]
    UnprocessableEntity(String),

    #[error("internal server error: {0}")]
    Internal(String),

    #[error("JWT error: {0}")]
    Jwt(String),

    #[error("bcrypt error: {0}")]
    Bcrypt(#[from] bcrypt::BcryptError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("storage error: {0}")]
    Storage(String),

    #[error("Shopify API error: {0}")]
    Shopify(String),

    #[error("Mailchimp API error: {0}")]
    Mailchimp(String),

    #[error("Safe Browsing error: {0}")]
    SafeBrowsing(String),

    #[error("Email error: {0}")]
    Email(String),

    #[error("migration failed: {0}")]
    MigrationFailed(String),

    #[error("schema out of sync: {0} migration(s) pending. {1}")]
    SchemaOutOfSync(usize, String),
}

impl actix_web::ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Config(_) => StatusCode::BAD_REQUEST,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::UnprocessableEntity(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Jwt(_) => StatusCode::UNAUTHORIZED,
            Self::Bcrypt(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Shopify(_) => StatusCode::BAD_GATEWAY,
            Self::Mailchimp(_) => StatusCode::BAD_GATEWAY,
            Self::SafeBrowsing(_) => StatusCode::BAD_GATEWAY,
            Self::Email(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::MigrationFailed(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::SchemaOutOfSync(_, _) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let msg = if cfg!(debug_assertions) {
            self.to_string()
        } else {
            match self {
                Self::Database(_) => "A database error occurred".to_string(),
                Self::Jwt(_) => "An authentication error occurred".to_string(),
                Self::Bcrypt(_) => "An internal error occurred".to_string(),
                _ => self.to_string(),
            }
        };
        HttpResponse::build(self.status_code()).json(serde_json::json!({
            "error": msg
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::ResponseError;

    #[test]
    fn config_error_returns_bad_request() {
        let err = AppError::Config(dotenvy::Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        )));
        assert_eq!(err.status_code(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn unauthorized_returns_401() {
        let err = AppError::Unauthorized;
        assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn forbidden_returns_403() {
        let err = AppError::Forbidden("must be admin".into());
        assert_eq!(err.status_code(), StatusCode::FORBIDDEN);
        assert_eq!(err.to_string(), "forbidden: must be admin");
    }

    #[test]
    fn not_found_returns_404() {
        let err = AppError::NotFound("user".into());
        assert_eq!(err.status_code(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn conflict_returns_409() {
        let err = AppError::Conflict("email already in use".into());
        assert_eq!(err.status_code(), StatusCode::CONFLICT);
    }

    #[test]
    fn validation_returns_422() {
        let err = AppError::Validation("invalid email".into());
        assert_eq!(err.status_code(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn jwt_error_returns_401() {
        let err = AppError::Jwt("invalid token".into());
        assert_eq!(err.status_code(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn error_response_returns_json() {
        let err = AppError::NotFound("campaign".into());
        let resp = err.error_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        let ct = resp.headers().get(actix_web::http::header::CONTENT_TYPE).unwrap();
        assert!(ct.to_str().unwrap().starts_with("application/json"));
    }

    #[test]
    fn from_dotenvy_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
        let dotenv_err = dotenvy::Error::Io(io_err);
        let app_err: AppError = dotenv_err.into();
        matches!(app_err, AppError::Config(_));
    }

    #[test]
    fn from_sqlx_error() {
        let sqlx_err = sqlx::Error::RowNotFound;
        let app_err: AppError = sqlx_err.into();
        matches!(app_err, AppError::Database(_));
    }

    #[test]
    fn shopify_error_returns_502() {
        let err = AppError::Shopify("GraphQL mutation failed".into());
        assert_eq!(err.status_code(), StatusCode::BAD_GATEWAY);
        assert_eq!(err.to_string(), "Shopify API error: GraphQL mutation failed");
    }

    #[test]
    fn mailchimp_error_returns_502() {
        let err = AppError::Mailchimp("API rate limit".into());
        assert_eq!(err.status_code(), StatusCode::BAD_GATEWAY);
        assert_eq!(err.to_string(), "Mailchimp API error: API rate limit");
    }

    #[test]
    fn safe_browsing_error_returns_502() {
        let err = AppError::SafeBrowsing("request failed".into());
        assert_eq!(err.status_code(), StatusCode::BAD_GATEWAY);
        assert_eq!(err.to_string(), "Safe Browsing error: request failed");
    }
}
