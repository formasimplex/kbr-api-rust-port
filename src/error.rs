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
}

impl actix_web::ResponseError for AppError {
    fn status_code(&self) -> StatusCode {
        match self {
            Self::Config(_) => StatusCode::BAD_REQUEST,
            Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::UnprocessableEntity(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Jwt(_) => StatusCode::UNAUTHORIZED,
            Self::Bcrypt(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponse::build(self.status_code()).json(serde_json::json!({
            "error": self.to_string()
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
        assert_eq!(err.to_string(), "not found: user");
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
}
