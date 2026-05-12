use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::album::{Album, AlbumResponse, CreateAlbumRequest};

pub async fn index() -> Result<HttpResponse, AppError> {
    let albums = mock_all_albums();
    let responses: Vec<AlbumResponse> = albums.iter().map(|a| a.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(path: web::Path<i64>) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_album(id) {
        Some(a) => Ok(HttpResponse::Ok().json(a.to_response())),
        None => Err(AppError::NotFound(format!("Album #{}", id))),
    }
}

pub async fn create(
    user: CurrentUser,
    body: web::Json<CreateAlbumRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let album = Album {
        id: 999,
        name: Some(body.name.clone()),
        release_date: body.release_date.clone(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(album.to_response()))
}

fn mock_all_albums() -> Vec<Album> {
    vec![Album {
        id: 1,
        name: Some("Debut Album".to_string()),
        release_date: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_find_album(id: i64) -> Option<Album> {
    if id == 1 {
        Some(Album {
            id: 1,
            name: Some("Debut Album".to_string()),
            release_date: None,
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
            .route("/albums", web::get().to(index))
            .route("/album/{id}", web::get().to(show))
            .route("/albums", web::post().to(create)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";

    #[tokio::test(flavor = "current_thread")]
    async fn albums_index_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/albums").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn album_show_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/album/1").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn album_show_not_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/album/9999").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn album_create_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let token = crate::auth::jwt::encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap();
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/albums")
            .insert_header(("Authorization", format!("Bearer {}", token)))
            .set_json(serde_json::json!({
                "name": "New Album"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }
}
