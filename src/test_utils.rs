//! Test utilities for building AppState in handler tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use sqlx::PgPool;

use crate::app::AppState;
use crate::auth::jwt::{Claims, encode_token_with_role};
use crate::services::storage_service::{S3Config, create_s3_bucket};

pub const TEST_SECRET: &str = "test-secret-key";

/// Generate a test JWT token for an admin user (user_id=1).
pub fn admin_token() -> String {
    encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap()
}

/// Generate a test JWT token for a regular user.
pub fn user_token(user_id: i64) -> String {
    encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap()
}

/// Generate a test JWT token for an artist user.
pub fn artist_token(user_id: i64) -> String {
    encode_token_with_role(user_id, TEST_SECRET, 3, Some("artist".to_string()), 1).unwrap()
}

/// Generate a test JWT token for a customer user.
pub fn customer_token(user_id: i64) -> String {
    encode_token_with_role(user_id, TEST_SECRET, 3, Some("customer".to_string()), 1).unwrap()
}

/// Generate a test JWT token with a custom role.
pub fn token_with_role(user_id: i64, role: &str) -> String {
    encode_token_with_role(user_id, TEST_SECRET, 3, Some(role.to_string()), 1).unwrap()
}

/// Global counter for generating unique suffixes in tests.
static TEST_COUNTER: AtomicI64 = AtomicI64::new(0);

/// Generate a unique suffix for test data (emails, names, etc.).
/// Uses timestamp + atomic counter to avoid collisions across modules.
pub fn unique_suffix() -> String {
    let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
    let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}_{}", ts, count)
}

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
pub async fn get_test_state() -> AppState {
    let pool = sqlx::PgPool::connect(&test_db_url())
        .await
        .expect("Failed to connect to test database");
    build_test_state(pool).await
}

/// Build a test AppState with the given pool.
pub async fn build_test_state(pool: PgPool) -> AppState {
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
