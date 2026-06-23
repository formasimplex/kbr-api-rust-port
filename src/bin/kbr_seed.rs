use std::env;

use chrono::NaiveDate;
use kbr_api_rust::db::pool;
use kbr_api_rust::services::auth_service::hash_password;
use uuid::Uuid;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let pool = match pool::connect().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to connect to database: {e}");
            std::process::exit(1);
        }
    };

    seed(&pool).await;
}

fn get_env(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| {
        eprintln!("Required environment variable not set: {key}");
        std::process::exit(1);
    })
}

async fn find_user_id(pool: &sqlx::PgPool, email: &str) -> Option<i64> {
    sqlx::query_scalar("SELECT id FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn find_artist_by_user_id(pool: &sqlx::PgPool, user_id: i64) -> Option<i64> {
    sqlx::query_scalar("SELECT id FROM artists WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn find_producer_id(pool: &sqlx::PgPool, name: &str) -> Option<i64> {
    sqlx::query_scalar("SELECT id FROM producers WHERE producer_name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn find_subscriber_id(
    pool: &sqlx::PgPool,
    email: &str,
    artist_id: Option<i64>,
) -> Option<i64> {
    if let Some(aid) = artist_id {
        sqlx::query_scalar(
            "SELECT id FROM mail_subscribers WHERE email = $1 AND artist_id = $2",
        )
        .bind(email)
        .bind(aid)
        .fetch_optional(pool)
        .await
        .unwrap()
    } else {
        sqlx::query_scalar(
            "SELECT id FROM mail_subscribers WHERE email = $1 AND artist_id IS NULL",
        )
        .bind(email)
        .fetch_optional(pool)
        .await
        .unwrap()
    }
}

async fn find_event_id(pool: &sqlx::PgPool, name: &str) -> Option<i64> {
    sqlx::query_scalar("SELECT id FROM kbr_events WHERE name = $1")
        .bind(name)
        .fetch_optional(pool)
        .await
        .unwrap()
}

async fn find_attendee_id(
    pool: &sqlx::PgPool,
    event_id: i64,
    subscriber_id: i64,
) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT id FROM kbr_event_attendees WHERE kbr_event_id = $1 AND mail_subscriber_id = $2",
    )
    .bind(event_id as i32)
    .bind(subscriber_id as i32)
    .fetch_optional(pool)
    .await
    .unwrap()
}

async fn find_tenant_config(pool: &sqlx::PgPool, tenant_id: Uuid) -> Option<uuid::Uuid> {
    sqlx::query_scalar("SELECT tenant_id FROM tenant_configs WHERE tenant_id = $1")
        .bind(tenant_id)
        .fetch_optional(pool)
        .await
        .unwrap()
}

// --- Producers ---

async fn seed_producer(pool: &sqlx::PgPool, name: &str, description: &str) -> i64 {
    if let Some(id) = find_producer_id(pool, name).await {
        let existing = sqlx::query_scalar::<_, Option<String>>(
            "SELECT description FROM producers WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(pool)
        .await
        .unwrap()
        .flatten();

        if existing.as_deref() != Some(description) {
            sqlx::query("UPDATE producers SET description = $2, updated_at = NOW() WHERE id = $1")
                .bind(id)
                .bind(description)
                .execute(pool)
                .await
                .unwrap();
        }
        return id;
    }

    sqlx::query_scalar(
        "INSERT INTO producers (producer_name, description, created_at, updated_at)
         VALUES ($1, $2, NOW(), NOW()) RETURNING id",
    )
    .bind(name)
    .bind(description)
    .fetch_one(pool)
    .await
    .unwrap()
}

// --- Users ---

async fn seed_user(
    pool: &sqlx::PgPool,
    email: &str,
    username: &str,
    role: &str,
    password: &str,
) -> i64 {
    if let Some(id) = find_user_id(pool, email).await {
        return id;
    }

    let password_digest = hash_password(password).expect("Failed to hash password");

    sqlx::query_scalar(
        "INSERT INTO users (email, password_digest, role, username, created_at, updated_at)
         VALUES ($1, $2, $3, $4, NOW(), NOW()) RETURNING id",
    )
    .bind(email)
    .bind(&password_digest)
    .bind(role)
    .bind(username)
    .fetch_one(pool)
    .await
    .unwrap()
}

// --- Artist ---

async fn seed_artist(
    pool: &sqlx::PgPool,
    user_id: i64,
    name: &str,
    genre: &str,
    bio: &str,
) -> i64 {
    if let Some(id) = find_artist_by_user_id(pool, user_id).await {
        return id;
    }

    sqlx::query_scalar(
        "INSERT INTO artists (name, genre, bio, user_id, created_at, updated_at)
         VALUES ($1, $2, $3, $4, NOW(), NOW()) RETURNING id",
    )
    .bind(name)
    .bind(genre)
    .bind(bio)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

// --- MailSubscribers ---

async fn seed_subscriber(
    pool: &sqlx::PgPool,
    full_name: &str,
    email: &str,
    artist_id: Option<i64>,
) -> i64 {
    if let Some(id) = find_subscriber_id(pool, email, artist_id).await {
        return id;
    }

    if let Some(aid) = artist_id {
        sqlx::query_scalar(
            "INSERT INTO mail_subscribers (full_name, email, artist_id, created_at, updated_at)
             VALUES ($1, $2, $3, NOW(), NOW()) RETURNING id",
        )
        .bind(full_name)
        .bind(email)
        .bind(aid)
        .fetch_one(pool)
        .await
        .unwrap()
    } else {
        sqlx::query_scalar(
            "INSERT INTO mail_subscribers (full_name, email, created_at, updated_at)
             VALUES ($1, $2, NOW(), NOW()) RETURNING id",
        )
        .bind(full_name)
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap()
    }
}

// --- KbrEvent ---

async fn seed_event(
    pool: &sqlx::PgPool,
    name: &str,
    description: &str,
    event_url: &str,
    start_date: NaiveDate,
    end_date: NaiveDate,
    create_by_user_id: i32,
) -> i64 {
    if let Some(id) = find_event_id(pool, name).await {
        return id;
    }

    sqlx::query_scalar(
        "INSERT INTO kbr_events (name, description, event_url, event_start_date, event_end_date, create_by_user_id, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW()) RETURNING id",
    )
    .bind(name)
    .bind(description)
    .bind(event_url)
    .bind(start_date.and_hms_opt(0, 0, 0).unwrap())
    .bind(end_date.and_hms_opt(0, 0, 0).unwrap())
    .bind(create_by_user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

// --- KbrEventAttendee ---

async fn seed_attendee(pool: &sqlx::PgPool, event_id: i64, subscriber_id: i64, scan_count: i32) {
    if let Some(_id) = find_attendee_id(pool, event_id, subscriber_id).await {
        return;
    }

    sqlx::query(
        "INSERT INTO kbr_event_attendees (kbr_event_id, mail_subscriber_id, scan_count, created_at, updated_at)
         VALUES ($1, $2, $3, NOW(), NOW())",
    )
    .bind(event_id as i32)
    .bind(subscriber_id as i32)
    .bind(scan_count)
    .execute(pool)
    .await
    .unwrap();
}

// --- TenantConfig ---

async fn seed_tenant_config(
    pool: &sqlx::PgPool,
    tenant_id: Uuid,
    logo_url: &str,
    short_name: &str,
    long_name: &str,
    footer_logo_url: &str,
    contact_email: &str,
    site_header_description: &str,
) {
    if find_tenant_config(pool, tenant_id).await.is_some() {
        return;
    }

    sqlx::query(
        "INSERT INTO tenant_configs (tenant_id, logo_url, short_name, long_name, footer_logo_url, contact_email, site_header_description, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())",
    )
    .bind(tenant_id)
    .bind(logo_url)
    .bind(short_name)
    .bind(long_name)
    .bind(footer_logo_url)
    .bind(contact_email)
    .bind(site_header_description)
    .execute(pool)
    .await
    .unwrap();
}

// --- Main seed function ---

async fn seed(pool: &sqlx::PgPool) {
    println!("Seeding data...");

    // 1. Producers
    println!("  Seeding producers...");
    seed_producer(pool, "Shopify", "Online store for selling merch").await;
    seed_producer(pool, "TPop", "Online producer of garments").await;

    // 2. Users
    println!("  Seeding users...");
    let will_password = get_env("WILL_ADMIN");
    let george_password = get_env("GEORGE_ADMIN");

    let admin_id = seed_user(
        pool,
        "will.simpson85@gmail.com",
        "WILL_ADMIN",
        "admin",
        &will_password,
    )
    .await;

    seed_user(
        pool,
        "georgesimpson204@gmail.com",
        "George",
        "admin",
        &george_password,
    )
    .await;

    seed_user(
        pool,
        "artist@kushtybuckrecords.com",
        "artist",
        "artist",
        "p455w0rd",
    )
    .await;

    // 3. Artist (linked to main admin)
    println!("  Seeding artist...");
    let _artist_id = seed_artist(pool, admin_id, "DJ D1", "House", "Lively bouncing tunes from the master do 1 DJ").await;

    // 4. MailSubscribers
    println!("  Seeding mail subscribers...");
    let sub_one_id = seed_subscriber(pool, "Will Simpson", "someone@somewhere.com", None).await;

    let dj_d1_artist_id = sqlx::query_scalar::<_, i64>("SELECT id FROM artists WHERE name = $1")
        .bind("DJ D1")
        .fetch_one(pool)
        .await
        .unwrap();

    seed_subscriber(
        pool,
        "Jane fawn",
        "jane@fawn.com",
        Some(dj_d1_artist_id),
    )
    .await;

    seed_subscriber(pool, "John Doe", "john@doe.com", None).await;

    // 5. Test Event
    println!("  Seeding test event...");
    let event_id = seed_event(
        pool,
        "Test Event",
        "This is a test event",
        "https://example.com/events/",
        NaiveDate::from_ymd_opt(2021, 1, 1).unwrap(),
        NaiveDate::from_ymd_opt(2021, 1, 2).unwrap(),
        admin_id as i32,
    )
    .await;

    // 6. Event Attendee (scan)
    println!("  Seeding event attendee...");
    seed_attendee(pool, event_id, sub_one_id, 1).await;

    // 7. Tenant Config
    println!("  Seeding tenant config...");
    seed_tenant_config(
        pool,
        Uuid::nil(),
        "https://example.com/kbr-logo.png",
        "KBR",
        "Keep Businesses Running",
        "https://example.com/kbr-footer-logo.png",
        "contact@kbr.com",
        "Empowering artists to connect with their fans",
    )
    .await;

    println!("Seed data loaded successfully — no duplicate rows were created");
}
