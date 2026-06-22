//! Test utilities for building AppState in handler tests.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::OnceLock;

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

/// Generate an ID that doesn't exist in the given table.
/// Queries MAX(id) and adds 9999 to ensure the ID is not found.
pub async fn not_found_id(pool: &PgPool, table: &str) -> i64 {
    let id: i64 = sqlx::query_scalar(&format!(
        r"SELECT COALESCE(MAX(id), 0) FROM {table}"
    ))
    .fetch_one(pool)
    .await
    .expect("Failed to get max id");
    id + 9999
}

/// Truncate all user tables in the test database.
/// Queries information_schema for tables in the public schema,
/// then runs TRUNCATE ... CASCADE to clear all data.
/// Retries on deadlock (40P01) as other connections may hold locks.
pub async fn truncate_all(pool: &PgPool) {
    let tables: Vec<String> = sqlx::query_scalar(
        r"SELECT table_name FROM information_schema.tables
           WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
           ORDER BY table_name",
    )
    .fetch_all(pool)
    .await
    .expect("Failed to query tables for truncation");

    if tables.is_empty() {
        return;
    }

    let table_list = tables.join(", ");
    for attempt in 0..5 {
        match sqlx::query(&format!(r"TRUNCATE TABLE {} CASCADE", table_list))
            .execute(pool)
            .await
        {
            Ok(_) => return,
            Err(e) if e.as_database_error().map(|e| e.code().as_deref() == Some("40P01")).unwrap_or(false) => {
                // Deadlock detected — wait and retry
                tokio::time::sleep(std::time::Duration::from_millis(50 * (attempt as u64 + 1))).await;
            }
            Err(e) => panic!("Failed to truncate tables: {e}"),
        }
    }
    panic!("Failed to truncate tables after 5 retries (deadlock)");
}

/// Global mutex that serializes all database-backed tests.
/// Each TestGuard acquires this lock for its entire lifetime,
/// preventing any other test from running concurrently.
static TEST_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

/// Guard that serializes and isolates database-backed tests.
/// Acquires a global mutex on creation (no other test can run concurrently),
/// truncates all tables, holds the lock until cleanup, then truncates again.
///
/// # Usage
/// ```ignore
/// let guard = TestGuard::new(&state.db).await;
/// // ... seed data, run tests ...
/// guard.cleanup().await;
/// ```
pub struct TestGuard {
    pool: PgPool,
    _lock: tokio::sync::MutexGuard<'static, ()>,
}

impl TestGuard {
    /// Acquire the global test mutex, truncate all tables, and return the guard.
    pub async fn new(pool: &PgPool) -> Self {
        let mutex = TEST_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()));
        let lock = mutex.lock().await;
        truncate_all(pool).await;
        Self {
            pool: pool.clone(),
            _lock: lock,
        }
    }

    pub async fn cleanup(self) {
        truncate_all(&self.pool).await;
        // _lock is dropped when self is dropped, releasing the mutex
    }
}

impl fmt::Debug for TestGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestGuard").finish()
    }
}

/// Seed a test user with a given email and role.
/// Uses a dummy password digest (not suitable for auth tests that need real hashing).
/// Returns the inserted user's ID.
pub async fn seed_user(pool: &PgPool, email: &str, role: &str) -> i64 {
    let now = chrono::Utc::now().naive_utc();
    sqlx::query_scalar::<_, i64>(
        r"INSERT INTO users (email, password_digest, role, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $4) RETURNING id"
    )
    .bind(email)
    .bind("hashed_password_test".to_string())
    .bind(Some(role.to_string()))
    .bind(&now)
    .fetch_one(pool)
    .await
    .expect("Failed to seed user")
}

/// Seed a user with a specific ID. Useful when tests use JWTs with hardcoded
/// user IDs (e.g., admin_token = user_id 1). Idempotent via ON CONFLICT.
pub async fn seed_user_with_id(pool: &PgPool, id: i64, email: &str, role: &str) -> i64 {
    let now = chrono::Utc::now().naive_utc();
    let _ = sqlx::query(
        r"INSERT INTO users (id, email, password_digest, role, created_at, updated_at)
           VALUES ($1, $2, '$2b$12$test', $3, $4, $4)
           ON CONFLICT (id) DO NOTHING"
    )
    .bind(id)
    .bind(email)
    .bind(role)
    .bind(&now)
    .execute(pool)
    .await;
    id
}

/// Delete a test user by email.
pub async fn cleanup_user(pool: &PgPool, email: &str) {
    let _ = sqlx::query(r"DELETE FROM users WHERE email = $1")
        .bind(email)
        .execute(pool)
        .await;
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
/// Uses a single-connection pool to eliminate concurrent connection issues.
pub async fn get_test_state() -> AppState {
    let pool = sqlx::pool::PoolOptions::new()
        .max_connections(1)
        .connect(&test_db_url())
        .await
        .expect("Failed to connect to test database");
    build_test_state(pool).await
}

/// Run test migrations once per process. Idempotent schema.sql ensures safety.
static TEST_MIGRATED: OnceLock<()> = OnceLock::new();

async fn ensure_migrations(pool: &PgPool) {
    if TEST_MIGRATED.get().is_none() {
        let _ = crate::db::migrate::run_migrations(pool).await;
        TEST_MIGRATED.set(());
    }
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
