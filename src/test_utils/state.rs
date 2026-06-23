use std::sync::Arc;

use sqlx::PgPool;

use crate::app::AppState;
use crate::auth::jwt::Claims;
use crate::services::storage::{S3Config, create_s3_bucket};
use super::guards::ensure_migrations;
use super::tokens::TEST_SECRET;

/// Get the test database URL from the `TEST_DB_URL` environment variable.
/// Falls back to `"postgresql://ws@localhost:5432/kbr_test"` if not set.
pub fn test_db_url() -> String {
    std::env::var("TEST_DB_URL").unwrap_or_else(|_| "postgresql://ws@localhost:5432/kbr_test".to_string())
}

/// Set DATABASE_URL for tests.
pub fn set_test_env() {
    unsafe { std::env::set_var("DATABASE_URL", test_db_url()); }
}

/// Set DATABASE_URL and JWT_SECRET for tests that need auth.
pub fn set_test_env_jwt() {
    unsafe {
        std::env::set_var("DATABASE_URL", test_db_url());
        std::env::set_var("JWT_SECRET", TEST_SECRET);
    }
}

/// Connect to the test database and build a full AppState.
/// Uses a single-connection pool to eliminate concurrent connection issues.
pub async fn get_test_state() -> AppState {
    let pool = sqlx::pool::PoolOptions::new()
        .max_connections(1)
        .connect(&test_db_url())
        .await
        .expect("Failed to connect to test database");
    build_test_state(pool).await
}

/// Build a test AppState with the given pool.
pub async fn build_test_state(pool: PgPool) -> AppState {
    ensure_migrations(&pool).await;
    let config = S3Config::from_env().unwrap_or_else(|_| S3Config {
        access_key: "test".to_string(),
        secret_key: "test".to_string(),
        endpoint: "https://test.test".to_string(),
        bucket_name: "test".to_string(),
        region: "us-east-1".to_string(),
    });
    let s3 = create_s3_bucket(&config).unwrap_or_else(|_| {
        let creds = s3::creds::Credentials::new(Some("test"), Some("test"), None, None, None).unwrap();
        s3::bucket::Bucket::new("test", s3::region::Region::Custom { region: "us-east-1".to_string(), endpoint: "https://test.test".to_string() }, creds)
            .unwrap()
            .with_path_style()
    });

    let cookie_builder = Arc::new(
        actix_jc::ActixJwtCookie::<Claims>::new()
            .cookie_name("jwt_cookie")
            .jwt_key(TEST_SECRET)
            .expiration(3 * 24 * 60 * 60)
    );

    AppState {
        db: pool,
        s3,
        shopify: None,
        mailchimp: None,
        safe_browsing: None,
        email: None,
        job_handle: crate::jobs::JobHandle::inline(),
        jwt_secret: TEST_SECRET.to_string(),
        cookie_builder,
    }
}

/// Build a test app with the given route configuration.
/// Expands to: create state + init_service, returns (guard, state, app).
/// The guard auto-truncates on creation and provides cleanup on drop.
///
/// # Example
/// ```ignore
/// let (guard, state, app) = build_test_app!(config_routes);
/// // ... seed data, run tests ...
/// guard.cleanup().await;
/// ```
#[macro_export]
macro_rules! build_test_app {
    ($routes:expr) => {{
        let state = actix_web::web::Data::new(crate::test_utils::get_test_state().await);
        let guard = crate::test_utils::TestGuard::new(&state.db).await;
        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .configure($routes),
        )
        .await;
        (guard, state, app)
    }};
}

/// Build test app with dual app_data: AppState + cookie builder.
/// Used by auth tests that need cookie-based session handling.
/// Returns (guard, state, app).
#[macro_export]
macro_rules! build_test_app_with_cookies {
    ($routes:expr) => {{
        let state = actix_web::web::Data::new(crate::test_utils::get_test_state().await);
        let guard = crate::test_utils::TestGuard::new(&state.db).await;
        let cookie_builder = state.cookie_builder.clone();
        let app = actix_web::test::init_service(
            actix_web::App::new()
                .app_data(state.clone())
                .app_data(actix_web::web::Data::new(cookie_builder))
                .configure($routes),
        )
        .await;
        (guard, state, app)
    }};
}
