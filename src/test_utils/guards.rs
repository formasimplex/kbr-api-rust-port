use std::fmt;
use std::sync::OnceLock;

use sqlx::PgPool;

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
    }
}

impl fmt::Debug for TestGuard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestGuard").finish()
    }
}

/// Run test migrations once per process. Idempotent schema.sql ensures safety.
static TEST_MIGRATED: OnceLock<()> = OnceLock::new();

pub(super) async fn ensure_migrations(pool: &PgPool) {
    if TEST_MIGRATED.get().is_none() {
        let _ = crate::db::migrate::run_migrations(pool).await;
        let _ = TEST_MIGRATED.set(());
    }
}

