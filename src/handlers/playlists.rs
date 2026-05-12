use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::news_playlist::{
    CreateNewsPlaylistRequest, NewsPlaylist, NewsPlaylistResponse, UpdateNewsPlaylistRequest,
};

pub async fn index_admin(
    user: CurrentUser,
    query: web::Query<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
     let playlists = if query.0.get("user_id").and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse::<i64>().ok()))).is_some() {
        mock_all_playlists()
    } else {
        mock_all_playlists()
    };
    let responses: Vec<NewsPlaylistResponse> = playlists.iter().map(|p| p.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show_admin(
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();
    match mock_find_playlist(id) {
        Some(p) => Ok(HttpResponse::Ok().json(p.to_response())),
        None => Err(AppError::NotFound(format!("Playlist #{}", id))),
    }
}

pub async fn destroy_admin(
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();
    match mock_find_playlist(id) {
        Some(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Playlist #{} deleted", id)
        }))),
        None => Err(AppError::NotFound(format!("Playlist #{}", id))),
    }
}

pub async fn dashboard_index(user: CurrentUser) -> Result<HttpResponse, AppError> {
    let playlists = mock_user_playlists(user.id);
    let responses: Vec<NewsPlaylistResponse> = playlists.iter().map(|p| p.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn dashboard_show(
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_playlist(id) {
        Some(p) => {
            if p.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
            Ok(HttpResponse::Ok().json(p.to_response()))
        }
        None => Err(AppError::NotFound(format!("Playlist #{}", id))),
    }
}

pub async fn dashboard_create(
    user: CurrentUser,
    body: web::Json<CreateNewsPlaylistRequest>,
) -> Result<HttpResponse, AppError> {
    if body.name.is_empty() {
        return Err(AppError::Validation("Playlist name is required".to_string()));
    }
    let playlist = NewsPlaylist {
        id: 999,
        user_id: user.id,
        name: body.name.clone(),
        description: body.description.clone(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(playlist.to_response()))
}

pub async fn dashboard_update(
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateNewsPlaylistRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_playlist(id) {
        Some(mut p) => {
            if p.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
            if let Some(ref name) = body.name {
                p.name = name.clone();
            }
            if let Some(ref desc) = body.description {
                p.description = Some(desc.clone());
            }
            Ok(HttpResponse::Ok().json(p.to_response()))
        }
        None => Err(AppError::NotFound(format!("Playlist #{}", id))),
    }
}

pub async fn dashboard_destroy(
    user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_playlist(id) {
        Some(p) => {
            if p.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
            Ok(HttpResponse::Ok().json(serde_json::json!({
                "message": format!("Playlist #{} deleted", id)
            })))
        }
        None => Err(AppError::NotFound(format!("Playlist #{}", id))),
    }
}

pub async fn dashboard_add_news(
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let playlist_id = path.into_inner();
    match mock_find_playlist(playlist_id) {
        Some(p) => {
            if p.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
        }
        None => return Err(AppError::NotFound(format!("Playlist #{}", playlist_id))),
    }
    let news_id = body.get("news_id").and_then(|v| v.as_i64()).ok_or_else(|| {
        AppError::Validation("news_id is required".to_string())
    })?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("News #{} added to playlist #{}", news_id, playlist_id)
    })))
}

pub async fn dashboard_reorder(
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let playlist_id = path.into_inner();
    match mock_find_playlist(playlist_id) {
        Some(p) => {
            if p.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
        }
        None => return Err(AppError::NotFound(format!("Playlist #{}", playlist_id))),
    }
    let _news_ids = body.get("news_ids").and_then(|v| v.as_array()).ok_or_else(|| {
        AppError::Validation("news_ids array is required".to_string())
    })?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "Playlist reordered"
    })))
}

pub async fn dashboard_remove_news(
    user: CurrentUser,
    path: web::Path<(i64, i64)>,
) -> Result<HttpResponse, AppError> {
    let (playlist_id, news_id) = path.into_inner();
    match mock_find_playlist(playlist_id) {
        Some(p) => {
            if p.user_id != user.id && !user.is_admin() {
                return Err(AppError::Forbidden("Not Authorized".to_string()));
            }
        }
        None => return Err(AppError::NotFound(format!("Playlist #{}", playlist_id))),
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("News #{} removed from playlist #{}", news_id, playlist_id)
    })))
}

fn mock_all_playlists() -> Vec<NewsPlaylist> {
    vec![NewsPlaylist {
        id: 1,
        user_id: 1,
        name: "My Favorites".to_string(),
        description: Some("Best articles".to_string()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_user_playlists(user_id: i64) -> Vec<NewsPlaylist> {
    mock_all_playlists()
        .into_iter()
        .filter(|p| p.user_id == user_id)
        .collect()
}

fn mock_find_playlist(id: i64) -> Option<NewsPlaylist> {
    if id == 1 {
        Some(NewsPlaylist {
            id: 1,
            user_id: 1,
            name: "My Favorites".to_string(),
            description: Some("Best articles".to_string()),
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
            .route("/admin/news_playlists", web::get().to(index_admin))
            .route("/admin/news_playlists/{id}", web::get().to(show_admin))
            .route("/admin/news_playlists/{id}", web::delete().to(destroy_admin))
            .route("/dashboard/news_playlists", web::get().to(dashboard_index))
            .route("/dashboard/news_playlists/{id}", web::get().to(dashboard_show))
            .route("/dashboard/news_playlists", web::post().to(dashboard_create))
            .route("/dashboard/news_playlists/{id}", web::put().to(dashboard_update))
            .route("/dashboard/news_playlists/{id}", web::delete().to(dashboard_destroy))
            .route("/dashboard/news_playlists/{id}/add_news", web::post().to(dashboard_add_news))
            .route("/dashboard/news_playlists/{id}/reorder", web::post().to(dashboard_reorder))
            .route(
                "/dashboard/news_playlists/{playlist_id}/remove_news/{news_id}",
                web::post().to(dashboard_remove_news),
            ),
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
    async fn admin_playlists_index() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/admin/news_playlists")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_playlists_index() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/dashboard/news_playlists")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_create_playlist() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/dashboard/news_playlists")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "name": "New Playlist",
                "description": "My new playlist"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dashboard_create_empty_name() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/dashboard/news_playlists")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "name": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }
}
