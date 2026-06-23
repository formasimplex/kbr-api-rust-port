use std::sync::atomic::{AtomicI64, Ordering};

use sqlx::PgPool;

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

/// Seed a test user with a unique email derived from the given prefix.
/// Returns (user_id, email).
pub async fn seed_test_user(pool: &PgPool, prefix: &str, role: &str) -> (i64, String) {
    let email = format!("{}{}@test.com", prefix, unique_suffix());
    let id = seed_user(pool, &email, role).await;
    (id, email)
}

/// Seed an artist with optional associated user.
/// Returns the artist's ID.
pub async fn seed_artist(pool: &PgPool, user_id: Option<i64>) -> i64 {
    let name = format!("Test Artist {}", unique_suffix());
    sqlx::query_scalar::<_, i64>(
        r"INSERT INTO artists (name, genre, bio, user_id, prospect, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
           RETURNING id"
    )
    .bind(&name)
    .bind(Some("Electronic".to_string()))
    .bind(Some("A test artist".to_string()))
    .bind(user_id)
    .bind(Some(false))
    .fetch_one(pool)
    .await
    .expect("Failed to seed artist")
}

/// Seed a campaign for the given artist.
/// Returns the campaign's ID.
pub async fn seed_campaign(pool: &PgPool, artist_id: i64) -> i64 {
    let name = format!("Test Campaign {}", unique_suffix());
    sqlx::query_scalar::<_, i64>(
        r"INSERT INTO campaigns (artist_id, name, active, vinyl_sold_count, progress, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, NOW(), NOW())
           RETURNING id"
    )
    .bind(artist_id)
    .bind(&name)
    .bind(true)
    .bind(25_i32)
    .bind(50_i32)
    .fetch_one(pool)
    .await
    .expect("Failed to seed campaign")
}

/// Seed a campaign page for the given campaign.
/// Returns the page's ID.
pub async fn seed_campaign_page(pool: &PgPool, campaign_id: i64) -> i64 {
    let title = format!("Test Page {}", unique_suffix());
    sqlx::query_scalar::<_, i64>(
        r"INSERT INTO campaign_pages (campaign_id, title, description, page_type, created_at, updated_at)
           VALUES ($1, $2, $3, $4, NOW(), NOW())
           RETURNING id"
    )
    .bind(campaign_id)
    .bind(&title)
    .bind(Some("A test campaign page".to_string()))
    .bind(Some(0_i32))
    .fetch_one(pool)
    .await
    .expect("Failed to seed campaign page")
}

/// Seed a mail subscriber with optional user and artist associations.
/// Returns the subscriber's ID.
pub async fn seed_mail_subscriber(
    pool: &PgPool,
    user_id: Option<i64>,
    artist_id: Option<i64>,
) -> i64 {
    let suffix = unique_suffix();
    let email = format!("sub_{}@test.com", suffix);
    sqlx::query_scalar::<_, i64>(
        r"INSERT INTO mail_subscribers (full_name, email, user_id, artist_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, NOW(), NOW())
           RETURNING id"
    )
    .bind(format!("Test Subscriber {}", suffix))
    .bind(&email)
    .bind(user_id)
    .bind(artist_id)
    .fetch_one(pool)
    .await
    .expect("Failed to seed mail subscriber")
}

/// Seed an event with optional creator user ID.
/// Returns the event's ID.
pub async fn seed_event(pool: &PgPool, create_by_user_id: Option<i32>) -> i64 {
    let now = chrono::Utc::now().naive_utc();
    let name = format!("Test Event {}", unique_suffix());
    sqlx::query_scalar::<_, i64>(
        r"INSERT INTO kbr_events (name, description, active, event_start_date, event_end_date, create_by_user_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id"
    )
    .bind(&name)
    .bind("A test event")
    .bind(true)
    .bind(&now)
    .bind(&now)
    .bind(create_by_user_id)
    .bind(&now)
    .bind(&now)
    .fetch_one(pool)
    .await
    .expect("Failed to seed event")
}

/// Seed an event attendee.
/// Returns the attendee's ID.
pub async fn seed_attendee(pool: &PgPool, event_id: i64, subscriber_id: i64, scan_count: i32) -> i64 {
    sqlx::query_scalar::<_, i64>(
        r"INSERT INTO kbr_event_attendees (kbr_event_id, mail_subscriber_id, scan_count, created_at, updated_at)
           VALUES ($1, $2, $3, NOW(), NOW())
           RETURNING id"
    )
    .bind(event_id as i32)
    .bind(subscriber_id as i32)
    .bind(scan_count)
    .fetch_one(pool)
    .await
    .expect("Failed to seed attendee")
}

/// Seed a news playlist. Ensures the user exists first.
/// Returns the playlist's ID.
pub async fn seed_news_playlist(pool: &PgPool, user_id: i64, name: &str) -> i64 {
    seed_user_with_id(pool, user_id, "admin@test.com", "admin").await;
    sqlx::query_scalar::<_, i64>(
        r"INSERT INTO news_playlists (user_id, name, description, created_at, updated_at)
           VALUES ($1, $2, $3, NOW(), NOW())
           RETURNING id"
    )
    .bind(user_id)
    .bind(name)
    .bind(Some(format!("Playlist desc for {}", name)))
    .fetch_one(pool)
    .await
    .expect("Failed to seed news playlist")
}
