use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::news::{CreateNewsRequest, News, NewsResponse, UpdateNewsRequest};

pub async fn index() -> Result<HttpResponse, AppError> {
    let news_items = mock_all_news();
    let responses: Vec<NewsResponse> = news_items.iter().map(|n| n.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(path: web::Path<i64>) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_news(id) {
        Some(n) => Ok(HttpResponse::Ok().json(n.to_response())),
        None => Err(AppError::NotFound(format!("News #{}", id))),
    }
}

pub async fn create(
    user: CurrentUser,
    body: web::Json<CreateNewsRequest>,
) -> Result<HttpResponse, AppError> {
    if !News::validate_url(&body.url) {
        return Err(AppError::Validation("Invalid or unsafe URL".to_string()));
    }
    let news = News {
        id: 999,
        url: Some(body.url.clone()),
        title: body.title.clone(),
        vote_score: Some(0),
        flagged: Some(false),
        flagged_at: None,
        user_id: user.id,
        image_url: body.image_url.clone(),
        active: Some(true),
        comments_enabled: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(news.to_response()))
}

pub async fn update(
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateNewsRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_news(id) {
        Some(mut n) => {
            if n.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
            if let Some(active) = body.active {
                n.active = Some(active);
            }
            if let Some(enabled) = body.comments_enabled {
                n.comments_enabled = enabled;
            }
            Ok(HttpResponse::Ok().json(n.to_response()))
        }
        None => Err(AppError::NotFound(format!("News #{}", id))),
    }
}

pub async fn toggle_comments(
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_news(id) {
        Some(mut n) => {
            if n.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
            n.comments_enabled = !n.comments_enabled;
            Ok(HttpResponse::Ok().json(n.to_response()))
        }
        None => Err(AppError::NotFound(format!("News #{}", id))),
    }
}

pub async fn add_to_playlist(
    user: CurrentUser,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let news_id = body.get("news_id").and_then(|v| v.as_i64()).ok_or_else(|| {
        AppError::Validation("news_id is required".to_string())
    })?;
    let playlist_id = body.get("playlist_id").and_then(|v| v.as_i64()).ok_or_else(|| {
        AppError::Validation("playlist_id is required".to_string())
    })?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("News #{} added to playlist #{}", news_id, playlist_id),
        "user_id": user.id
    })))
}

fn mock_all_news() -> Vec<News> {
    vec![News {
        id: 1,
        url: Some("https://example.com/article".to_string()),
        title: Some("Breaking News".to_string()),
        vote_score: Some(10),
        flagged: Some(false),
        flagged_at: None,
        user_id: 1,
        image_url: Some("https://example.com/img.png".to_string()),
        active: Some(true),
        comments_enabled: true,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_find_news(id: i64) -> Option<News> {
    if id == 1 {
        Some(News {
            id: 1,
            url: Some("https://example.com/article".to_string()),
            title: Some("Breaking News".to_string()),
            vote_score: Some(10),
            flagged: Some(false),
            flagged_at: None,
            user_id: 1,
            image_url: Some("https://example.com/img.png".to_string()),
            active: Some(true),
            comments_enabled: true,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        })
    } else {
        None
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/news", web::get().to(index))
            .route("/news/{id}", web::get().to(show))
            .route("/news", web::post().to(create))
            .route("/news/{id}", web::put().to(update))
            .route("/news/{id}/toggle_comments", web::post().to(toggle_comments))
            .route("/news/add_to_playlist", web::post().to(add_to_playlist)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";

    fn admin_token() -> String {
        crate::auth::jwt::encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_index_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/news").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_show_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/news/1").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_create_valid_url() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/news")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "url": "https://example.com/article"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_create_malicious_url() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/news")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "url": "http://localhost/admin"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn news_toggle_comments() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/news/1/toggle_comments")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }
}
