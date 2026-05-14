use actix_web::{web, HttpResponse};
use serde::Serialize;
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::services::storage_service;

#[derive(Debug, FromRow)]
struct SubscribedArtistRow {
    id: i64,
    name: Option<String>,
    intro: Option<String>,
}

#[derive(Debug, Serialize)]
struct SubscribedArtistResponse {
    id: i64,
    name: Option<String>,
    intro: Option<String>,
    image_thumbnail_urls: Vec<String>,
}

pub async fn subscribed_artists(
    user: CurrentUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let rows = sqlx::query_as::<_, SubscribedArtistRow>(
        r#"
        SELECT DISTINCT artists.id, artists.name, artists.intro
        FROM artists
        INNER JOIN mail_subscribers ON mail_subscribers.artist_id = artists.id
        WHERE mail_subscribers.user_id = $1
          AND mail_subscribers.unsubscribed_at IS NULL
        ORDER BY artists.id
        "#
    )
    .bind(user.id)
    .fetch_all(&state.db)
    .await?;

    let mut responses = Vec::new();
    for row in rows {
        let (_, thumbnail_urls) = storage_service::get_image_urls(
            &state.s3,
            &state.db,
            "Artist",
            row.id,
        )
        .await
        .unwrap_or_else(|_| (Vec::new(), Vec::new()));

        responses.push(SubscribedArtistResponse {
            id: row.id,
            name: row.name,
            intro: row.intro,
            image_thumbnail_urls: thumbnail_urls,
        });
    }

    Ok(HttpResponse::Ok().json(responses))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/dashboard/subscribed_artists", web::get().to(subscribed_artists)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token_with_role;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";
    const TEST_DB_URL: &str = "postgresql://ws@localhost:5432/kbr_test";

    async fn get_state() -> AppState {
        let pool = sqlx::PgPool::connect(TEST_DB_URL)
            .await
            .expect("Failed to connect to test database");

        let config = crate::services::storage_service::S3Config::from_env()
            .unwrap_or_else(|_| crate::services::storage_service::S3Config {
                access_key: "test".to_string(),
                secret_key: "test".to_string(),
                endpoint: "https://test.test".to_string(),
                bucket_name: "test".to_string(),
                region: "us-east-1".to_string(),
            });
        let s3 = crate::services::storage_service::create_s3_bucket(&config).unwrap_or_else(|_| {
            let creds = s3::creds::Credentials::new(Some("test"), Some("test"), None, None, None).unwrap();
            s3::bucket::Bucket::new("test", s3::region::Region::Custom { region: "us-east-1".to_string(), endpoint: "https://test.test".to_string() }, creds)
                .unwrap()
                .with_path_style()
        });

        AppState { db: pool, s3 }
    }

    fn user_token(user_id: i64) -> String {
        encode_token_with_role(user_id, TEST_SECRET, 1, Some("user".to_string())).unwrap()
    }

    async fn seed_user(pool: &sqlx::PgPool) -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO users (email, password_digest, role, created_at, updated_at)
               VALUES ($1, $2, $3, NOW(), NOW())
               RETURNING id"
        )
        .bind(format!("dash_test_{}@test.com", ts))
        .bind("hashed_password_test".to_string())
        .bind(Some("user".to_string()))
        .fetch_one(pool)
        .await
        .expect("Failed to seed user")
    }

    async fn seed_artist(pool: &sqlx::PgPool) -> i64 {
        let ts = chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0);
        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO artists (name, intro, created_at, updated_at)
               VALUES ($1, $2, NOW(), NOW())
               RETURNING id"
        )
        .bind(format!("Dash Artist {}", ts))
        .bind(Some("An intro for the dashboard artist".to_string()))
        .fetch_one(pool)
        .await
        .expect("Failed to seed artist")
    }

    async fn seed_mail_subscriber(pool: &sqlx::PgPool, user_id: i64, artist_id: i64) {
        let _ = sqlx::query(
            r"INSERT INTO mail_subscribers (full_name, user_id, email, artist_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, NOW(), NOW())"
        )
        .bind("Test Subscriber".to_string())
        .bind(user_id)
        .bind(format!("sub_{}@test.com", artist_id))
        .bind(artist_id)
        .execute(pool)
        .await;
    }

    async fn cleanup(pool: &sqlx::PgPool, user_id: i64, artist_ids: &[i64]) {
        let _ = sqlx::query(r"DELETE FROM mail_subscribers WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
        for artist_id in artist_ids {
            let _ = sqlx::query(r"DELETE FROM artists WHERE id = $1")
                .bind(artist_id)
                .execute(pool)
                .await;
        }
        let _ = sqlx::query(r"DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(pool)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn subscribed_artists_returns_subscribed() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let user_id = seed_user(&state.db).await;
        let artist_id = seed_artist(&state.db).await;
        seed_mail_subscriber(&state.db, user_id, artist_id).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/dashboard/subscribed_artists")
            .insert_header(("Authorization", format!("Bearer {}", user_token(user_id))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert!(!body.is_empty());
        let found = body.iter().any(|v| v["id"] == artist_id);
        assert!(found, "Expected artist {} in response", artist_id);

        cleanup(&state.db, user_id, &[artist_id]).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn subscribed_artists_excludes_unsubscribed() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let user_id = seed_user(&state.db).await;
        let artist_id = seed_artist(&state.db).await;
        seed_mail_subscriber(&state.db, user_id, artist_id).await;

        let _ = sqlx::query(r"UPDATE mail_subscribers SET unsubscribed_at = NOW() WHERE user_id = $1 AND artist_id = $2")
            .bind(user_id)
            .bind(artist_id)
            .execute(&state.db)
            .await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/dashboard/subscribed_artists")
            .insert_header(("Authorization", format!("Bearer {}", user_token(user_id))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        let found = body.iter().any(|v| v["id"] == artist_id);
        assert!(!found, "Unsubscribed artist should not appear");

        cleanup(&state.db, user_id, &[artist_id]).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn subscribed_artists_empty_when_no_subscriptions() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let user_id = seed_user(&state.db).await;

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/dashboard/subscribed_artists")
            .insert_header(("Authorization", format!("Bearer {}", user_token(user_id))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert!(body.is_empty());

        cleanup(&state.db, user_id, &[]).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn subscribed_artists_requires_auth() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }

        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/dashboard/subscribed_artists")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_ne!(resp.status(), 200);
    }
}
