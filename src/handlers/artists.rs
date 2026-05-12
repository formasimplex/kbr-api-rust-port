use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_artist_or_above;
use crate::error::AppError;
use crate::models::artist::{Artist, ArtistResponse, CreateArtistRequest, UpdateArtistRequest};
use crate::models::artist_link::{ArtistLink, CreateArtistLinkRequest};

pub async fn index() -> Result<HttpResponse, AppError> {
    let artists = mock_all_artists();
    let responses: Vec<ArtistResponse> = artists.iter().map(|a| a.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(path: web::Path<i64>) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_artist(id) {
        Some(a) => Ok(HttpResponse::Ok().json(a.to_response())),
        None => Err(AppError::NotFound(format!("Artist #{}", id))),
    }
}

pub async fn create(
    user: CurrentUser,
    body: web::Json<CreateArtistRequest>,
) -> Result<HttpResponse, AppError> {
    if !user.is_admin() {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    if let Some(ref intro) = body.intro {
        if !Artist::validate_intro(intro) {
            return Err(AppError::Validation("Intro must be 300 characters or less".to_string()));
        }
    }
    if let Some(ref bio) = body.bio {
        if !Artist::validate_bio(bio) {
            return Err(AppError::Validation("Bio must be 3000 characters or less".to_string()));
        }
    }
    let artist = Artist {
        id: 999,
        name: Some(body.name.clone()),
        genre: body.genre.clone(),
        bio: body.bio.clone(),
        user_id: None,
        prospect: body.prospect,
        spotify_id: body.spotify_id.clone(),
        sub_heading: body.sub_heading.clone(),
        intro: body.intro.clone(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(artist.to_response()))
}

pub async fn update(
    user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateArtistRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let id = path.into_inner();
    match mock_find_artist(id) {
        Some(mut a) => {
            if let Some(ref name) = body.name {
                a.name = Some(name.clone());
            }
            if let Some(ref genre) = body.genre {
                a.genre = Some(genre.clone());
            }
            if let Some(ref bio) = body.bio {
                if !Artist::validate_bio(bio) {
                    return Err(AppError::Validation("Bio must be 3000 characters or less".to_string()));
                }
                a.bio = Some(bio.clone());
            }
            if let Some(ref intro) = body.intro {
                if !Artist::validate_intro(intro) {
                    return Err(AppError::Validation("Intro must be 300 characters or less".to_string()));
                }
                a.intro = Some(intro.clone());
            }
            Ok(HttpResponse::Ok().json(a.to_response()))
        }
        None => Err(AppError::NotFound(format!("Artist #{}", id))),
    }
}

pub async fn add_artist_links(
    user: CurrentUser,
    body: web::Json<CreateArtistLinkRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    if !crate::models::artist_link::ArtistLink::validate_url(&body.url) {
        return Err(AppError::Validation("Invalid URL".to_string()));
    }
    let link = ArtistLink {
        id: 999,
        artist_id: body.artist_id,
        link_type: body.link_type,
        url: body.url.clone(),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(link.to_response()))
}

pub async fn delete_artist_links(
    user: CurrentUser,
    body: web::Json<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    if !is_artist_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    let link_id = body.get("id").and_then(|v| v.as_i64()).ok_or_else(|| {
        AppError::Validation("Link id is required".to_string())
    })?;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": format!("Artist link #{} deleted", link_id)
    })))
}

pub async fn available_link_types() -> Result<HttpResponse, AppError> {
    let types = crate::models::artist_link::ArtistLink::available_link_types();
    Ok(HttpResponse::Ok().json(types))
}

fn mock_all_artists() -> Vec<Artist> {
    vec![Artist {
        id: 1,
        name: Some("DJ Test".to_string()),
        genre: Some("Electronic".to_string()),
        bio: Some("A great DJ".to_string()),
        user_id: Some(1),
        prospect: Some(false),
        spotify_id: None,
        sub_heading: None,
        intro: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_find_artist(id: i64) -> Option<Artist> {
    if id == 1 {
        Some(Artist {
            id: 1,
            name: Some("DJ Test".to_string()),
            genre: Some("Electronic".to_string()),
            bio: Some("A great DJ".to_string()),
            user_id: Some(1),
            prospect: Some(false),
            spotify_id: None,
            sub_heading: None,
            intro: None,
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
            .route("/artists", web::get().to(index))
            .route("/artist/{id}", web::get().to(show))
            .route("/artist", web::post().to(create))
            .route("/artist/{id}", web::put().to(update))
            .route("/artist/add_artist_links", web::post().to(add_artist_links))
            .route("/artist/delete_artist_links", web::post().to(delete_artist_links))
            .route("/available_link_types", web::get().to(available_link_types)),
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
    async fn artists_index_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/artists").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_show_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/artist/1").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn artist_create_admin() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/artist")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "name": "New Artist",
                "genre": "Jazz"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn available_link_types_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/available_link_types").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }
}
