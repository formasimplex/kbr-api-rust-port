use std::env;

use kbr_api_rust::db::{migrate, pool};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args: Vec<String> = env::args().collect();
    let command = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    if command == "help" || command == "--help" || command == "-h" {
        print_help();
        std::process::exit(0);
    }

    let pool = match pool::connect().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to connect to database: {}", e);
            std::process::exit(1);
        }
    };

    let code = match command {
        "migrate" => cmd_migrate(&pool).await,
        "rollback" => cmd_rollback(&pool, &args).await,
        "status" => cmd_status(&pool).await,
        "check" => cmd_check(&pool).await,
        _ => {
            eprintln!("Unknown command: {}", command);
            print_help();
            1
        }
    };

    std::process::exit(code);
}

async fn cmd_migrate(pool: &sqlx::PgPool) -> i32 {
    match migrate::run_migrations(pool).await {
        Ok(()) => {
            println!("All migrations applied successfully");
            0
        }
        Err(e) => {
            eprintln!("Migration failed: {}", e);
            1
        }
    }
}

async fn cmd_rollback(pool: &sqlx::PgPool, args: &[String]) -> i32 {
    let steps: u32 = args
        .get(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);

    match migrate::rollback(pool, steps).await {
        Ok(()) => {
            println!("Rolled back {} migration(s)", steps);
            0
        }
        Err(e) => {
            eprintln!("Rollback failed: {}", e);
            1
        }
    }
}

async fn cmd_status(pool: &sqlx::PgPool) -> i32 {
    let applied = match migrate::get_applied_migrations_status(pool).await {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Failed to get migration status: {}", e);
            return 1;
        }
    };

    let pending = match migrate::check_health(pool).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to check health: {}", e);
            return 1;
        }
    };

    println!("Applied migrations ({}):", applied.len());
    for m in &applied {
        println!("  {} {}", m.version, m.description);
    }

    if !pending.is_empty() {
        println!("\nPending migrations ({}):", pending.len());
        for m in &pending {
            println!("  {} {}", m.version, m.description);
        }
    }

    0
}

async fn cmd_check(pool: &sqlx::PgPool) -> i32 {
    match migrate::ensure_schema(pool).await {
        Ok(()) => {
            println!("Schema is up to date");
            0
        }
        Err(e) => {
            eprintln!("Schema out of sync: {}", e);
            1
        }
    }
}

fn print_help() {
    println!(
        r#"kbr-migrate - Database migration tool

Usage:
  kbr-migrate <command> [options]

Commands:
  migrate              Run all pending migrations
  rollback [N]         Rollback the last N migration(s) (default: 1)
  status               Show applied and pending migrations
  check                Check if schema is up to date (exit 0/1)
  help                 Show this help message

Environment:
  DATABASE_URL         PostgreSQL connection string (required)"#
    );
}
