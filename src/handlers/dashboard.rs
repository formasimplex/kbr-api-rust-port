//! Dashboard handlers
//!
//! Provides dashboard-specific endpoints for authenticated users.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `subscribed_artists` | GET | `/v1/dashboard/subscribed_artists` | auth | List artists the user is subscribed to via mail |

use actix_web::{web, HttpResponse};
use serde::Serialize;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::data::dashboard as data;
use crate::error::AppError;
use crate::services::storage;

/// Response for a subscribed artist on the dashboard.
#[derive(Debug, Serialize)]
struct SubscribedArtistResponse {
    id: i64,
    name: Option<String>,
    intro: Option<String>,
    image_thumbnail_urls: Vec<String>,
}

/// List artists the authenticated user is subscribed to.
///
/// Returns artists that have an active mail subscription for the user
/// (unsubscribed_at IS NULL). Includes thumbnail image URLs from S3.
/// Requires authentication.
///
/// # Response
///
/// `200 OK` — JSON array of `SubscribedArtistResponse`
pub async fn subscribed_artists(
    user: CurrentUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let rows = data::subscribed_artists(&state.db, user.id).await?;

    let mut responses = Vec::new();
    for row in rows {
        let (_, thumbnail_urls) = storage::get_image_urls(
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
    cfg.route("/dashboard/subscribed_artists", web::get().to(subscribed_artists));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{seed_artist, seed_mail_subscriber, seed_test_user, user_token};
    use actix_web::test;

    #[tokio::test(flavor = "current_thread")]
    async fn subscribed_artists_returns_subscribed() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "dash_test", "user").await;
        let artist_id = seed_artist(&state.db, None).await;
        seed_mail_subscriber(&state.db, Some(user_id), Some(artist_id)).await;

        let req = test::TestRequest::get()
            .uri("/dashboard/subscribed_artists")
            .insert_header(("Authorization", format!("Bearer {}", user_token(user_id))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert!(!body.is_empty());
        let found = body.iter().any(|v| v["id"] == artist_id);
        assert!(found, "Expected artist {} in response", artist_id);

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn subscribed_artists_excludes_unsubscribed() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "dash_test", "user").await;
        let artist_id = seed_artist(&state.db, None).await;
        seed_mail_subscriber(&state.db, Some(user_id), Some(artist_id)).await;

        let _ = sqlx::query(r"UPDATE mail_subscribers SET unsubscribed_at = NOW() WHERE user_id = $1 AND artist_id = $2")
            .bind(user_id)
            .bind(artist_id)
            .execute(&state.db)
            .await;

        let req = test::TestRequest::get()
            .uri("/dashboard/subscribed_artists")
            .insert_header(("Authorization", format!("Bearer {}", user_token(user_id))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        let found = body.iter().any(|v| v["id"] == artist_id);
        assert!(!found, "Unsubscribed artist should not appear");

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn subscribed_artists_empty_when_no_subscriptions() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, state, app) = crate::build_test_app!(config_routes);

        let (user_id, _) = seed_test_user(&state.db, "dash_test", "user").await;

        let req = test::TestRequest::get()
            .uri("/dashboard/subscribed_artists")
            .insert_header(("Authorization", format!("Bearer {}", user_token(user_id))))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: Vec<serde_json::Value> = test::read_body_json(resp).await;
        assert!(body.is_empty());

        _guard.cleanup().await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn subscribed_artists_requires_auth() {
        crate::test_utils::set_test_env_jwt();
        let (_guard, _state, app) = crate::build_test_app!(config_routes);

        let req = test::TestRequest::get()
            .uri("/dashboard/subscribed_artists")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_ne!(resp.status(), 200);

        _guard.cleanup().await;
    }
}
