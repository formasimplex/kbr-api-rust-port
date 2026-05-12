use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::song::{CreateSongRequest, Song, SongResponse};

pub async fn index() -> Result<HttpResponse, AppError> {
    let songs = mock_all_songs();
    let responses: Vec<SongResponse> = songs.iter().map(|s| s.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(path: web::Path<i64>) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_song(id) {
        Some(s) => Ok(HttpResponse::Ok().json(s.to_response())),
        None => Err(AppError::NotFound(format!("Song #{}", id))),
    }
}

pub async fn create(
    user: CurrentUser,
    body: web::Json<CreateSongRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let song = Song {
        id: 999,
        name: Some(body.name.clone()),
        duration: body.duration.clone(),
        album_id: body.album_id,
        artist_id: body.artist_id,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(song.to_response()))
}

fn mock_all_songs() -> Vec<Song> {
    vec![Song {
        id: 1,
        name: Some("Hit Song".to_string()),
        duration: Some("3:45".to_string()),
        album_id: 1,
        artist_id: 1,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_find_song(id: i64) -> Option<Song> {
    if id == 1 {
        Some(Song {
            id: 1,
            name: Some("Hit Song".to_string()),
            duration: Some("3:45".to_string()),
            album_id: 1,
            artist_id: 1,
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
            .route("/songs", web::get().to(index))
            .route("/song/{id}", web::get().to(show))
            .route("/songs", web::post().to(create)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    #[tokio::test(flavor = "current_thread")]
    async fn songs_index_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/songs").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn song_show_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/song/1").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn song_show_not_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/song/9999").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }
}
