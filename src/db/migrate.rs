use sqlx::{migrate::Migrator, PgPool, Row};
use std::path::Path;

use crate::error::AppError;

/// Create a migrator from the migrations/ directory.
/// File I/O is cached by the OS, so this is fast on repeated calls.
async fn get_migrator() -> Migrator {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let migrations_dir = format!("{}/migrations", manifest_dir);
    let mut migrator = Migrator::new(Path::new(&migrations_dir))
        .await
        .expect("Failed to load migrations");
    migrator.set_locking(false);
    migrator
}

/// Run all pending migrations.
pub async fn run_migrations(pool: &PgPool) -> Result<(), AppError> {
    get_migrator()
        .await
        .run(pool)
        .await
        .map_err(|e| AppError::MigrationFailed(e.to_string()))?;
    tracing::info!("Database migrations completed successfully");
    Ok(())
}

/// Rollback the last N migrations (default: 1).
pub async fn rollback(pool: &PgPool, steps: u32) -> Result<(), AppError> {
    let applied = get_applied_migrations(pool).await?;

    if steps as usize > applied.len() {
        return Err(AppError::MigrationFailed(format!(
            "Cannot rollback {} step(s): only {} migration(s) applied",
            steps,
            applied.len()
        )));
    }

    let target_version = if steps as usize == applied.len() {
        0
    } else {
        let target_idx = applied.len() - steps as usize - 1;
        applied[target_idx].version
    };

    get_migrator()
        .await
        .undo(pool, target_version)
        .await
        .map_err(|e| AppError::MigrationFailed(e.to_string()))?;

    tracing::info!(
        target_version = target_version,
        steps = steps,
        "Rolled back migrations"
    );
    Ok(())
}

/// Information about a pending migration.
#[derive(Debug)]
pub struct PendingMigration {
    pub version: i64,
    pub description: String,
}

/// Information about an applied migration.
#[derive(Debug)]
pub struct AppliedMigrationStatus {
    pub version: i64,
    pub description: String,
}

/// Get the status of applied migrations (for the status command).
pub async fn get_applied_migrations_status(
    pool: &PgPool,
) -> Result<Vec<AppliedMigrationStatus>, AppError> {
    let migrations = sqlx::query(
        r#"
        SELECT version, description FROM _sqlx_migrations
        ORDER BY version ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::MigrationFailed(e.to_string()))?;

    Ok(migrations
        .into_iter()
        .map(|r| AppliedMigrationStatus {
            version: r.try_get("version").unwrap_or(0_i64),
            description: r.try_get("description").unwrap_or_default(),
        })
        .collect())
}

/// Query applied migrations from the _sqlx_migrations table.
async fn get_applied_migrations(pool: &PgPool) -> Result<Vec<AppliedMigration>, AppError> {
    let migrations = sqlx::query(
        r#"
        SELECT version FROM _sqlx_migrations
        ORDER BY version ASC
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::MigrationFailed(e.to_string()))?;

    Ok(migrations
        .into_iter()
        .map(|r| AppliedMigration {
            version: r.try_get("version").unwrap_or(0_i64),
        })
        .collect())
}

#[derive(Debug)]
struct AppliedMigration {
    version: i64,
}

/// Check migration health — returns list of pending migrations.
pub async fn check_health(pool: &PgPool) -> Result<Vec<PendingMigration>, AppError> {
    let applied = get_applied_migrations(pool).await?;

    let applied_versions: std::collections::HashSet<i64> =
        applied.iter().map(|m| m.version).collect();

    let migrator = get_migrator().await;
    let pending = migrator
        .iter()
        .filter(|m| !applied_versions.contains(&m.version) && m.migration_type.is_up_migration())
        .map(|m| PendingMigration {
            version: m.version,
            description: m.description.to_string(),
        })
        .collect();

    Ok(pending)
}

/// Check health and fail fast if schema is out of sync.
pub async fn ensure_schema(pool: &PgPool) -> Result<(), AppError> {
    let pending = check_health(pool).await?;
    if pending.is_empty() {
        return Ok(());
    }

    let descriptions = pending
        .iter()
        .map(|m| format!("{} ({})", m.version, m.description))
        .collect::<Vec<_>>()
        .join(", ");

    Err(AppError::SchemaOutOfSync(
        pending.len(),
        format!(
            "Run: cargo run --bin kbr-migrate -- migrate\nPending: {}",
            descriptions
        ),
    ))
}
