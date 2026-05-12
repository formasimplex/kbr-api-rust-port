use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::artist_merchandise::{
    ArtistMerchandise, ArtistMerchandiseResponse, CreateArtistMerchandiseRequest,
    UpdateArtistMerchandiseRequest,
};
use crate::models::shopify_json_cache::{ShopifyJsonCache, ShopifyJsonCacheResponse};

pub async fn index(_user: CurrentUser) -> Result<HttpResponse, AppError> {
    let items = mock_all_merchandise();
    let responses: Vec<ArtistMerchandiseResponse> = items.iter().map(|m| m.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn show(
    _user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_merchandise(id) {
        Some(m) => Ok(HttpResponse::Ok().json(m.to_response())),
        None => Err(AppError::NotFound(format!("Merchandise #{}", id))),
    }
}

pub async fn by_artist(
    _user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let artist_id = path.into_inner();
    let items = mock_all_merchandise()
        .into_iter()
        .filter(|m| m.artist_id == artist_id)
        .collect::<Vec<_>>();
    let responses: Vec<ArtistMerchandiseResponse> = items.iter().map(|m| m.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn create(
    _user: CurrentUser,
    body: web::Json<CreateArtistMerchandiseRequest>,
) -> Result<HttpResponse, AppError> {
    if body.merch_title.is_empty() {
        return Err(AppError::Validation("merch_title is required".to_string()));
    }
    let merch = ArtistMerchandise {
        id: 999,
        artist_id: body.artist_id,
        producer_id: body.producer_id,
        merchandise_id: body.merchandise_id.clone(),
        description: body.description.clone(),
        created_on_producer: body.created_on_producer,
        merch_title: body.merch_title.clone(),
        merch_product_title: body.merch_product_title.clone(),
        set_price: body.set_price,
        cost_price: body.cost_price,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(merch.to_response()))
}

pub async fn update(
    _user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateArtistMerchandiseRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_merchandise(id) {
        Some(mut m) => {
            if let Some(ref title) = body.merch_title {
                m.merch_title = title.clone();
            }
            if let Some(ref desc) = body.description {
                m.description = Some(desc.clone());
            }
            if let Some(price) = body.set_price {
                m.set_price = Some(price);
            }
            Ok(HttpResponse::Ok().json(m.to_response()))
        }
        None => Err(AppError::NotFound(format!("Merchandise #{}", id))),
    }
}

pub async fn destroy(
    _user: CurrentUser,
    path: web::Path<i64>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_merchandise(id) {
        Some(_) => Ok(HttpResponse::Ok().json(serde_json::json!({
            "message": format!("Merchandise #{} deleted", id)
        }))),
        None => Err(AppError::NotFound(format!("Merchandise #{}", id))),
    }
}

pub async fn cache_update(_user: CurrentUser) -> Result<HttpResponse, AppError> {
    let caches = mock_all_caches();
    let responses: Vec<ShopifyJsonCacheResponse> = caches.iter().map(|c| c.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

fn mock_all_merchandise() -> Vec<ArtistMerchandise> {
    vec![ArtistMerchandise {
        id: 1,
        artist_id: 1,
        producer_id: 1,
        merchandise_id: Some("shopify-1".to_string()),
        description: Some("Band Tee".to_string()),
        created_on_producer: Some(true),
        merch_title: "Band T-shirt".to_string(),
        merch_product_title: Some("Band Tee - Black".to_string()),
        set_price: Some(25.99),
        cost_price: Some(12.50),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_find_merchandise(id: i64) -> Option<ArtistMerchandise> {
    if id == 1 {
        mock_all_merchandise().into_iter().next()
    } else {
        None
    }
}

fn mock_all_caches() -> Vec<ShopifyJsonCache> {
    vec![ShopifyJsonCache {
        id: 1,
        cached_item_id: Some("product-1".to_string()),
        json_entry: Some(r#"{"title":"Product"}"#.to_string()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/merchandise", web::get().to(index))
            .route("/merchandise/cache_update", web::get().to(cache_update))
            .route("/artist_merchandise", web::get().to(index))
            .route("/artist_merchandise/{id}", web::get().to(show))
            .route("/artist_merchandise/by_artist/{artist_id}", web::get().to(by_artist))
            .route("/artist_merchandise", web::post().to(create))
            .route("/artist_merchandise/{id}", web::put().to(update))
            .route("/artist_merchandise/{id}", web::delete().to(destroy)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::jwt::encode_token;
    use actix_web::{test, App};

    const TEST_SECRET: &str = "test-secret-key";

    fn token() -> String {
        crate::auth::jwt::encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_index_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/artist_merchandise")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_by_artist() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::get()
            .uri("/v1/artist_merchandise/by_artist/1")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_create_success() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/artist_merchandise")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .set_json(serde_json::json!({
                "artist_id": 1,
                "producer_id": 1,
                "merch_title": "New Item"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn merchandise_create_empty_title() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/artist_merchandise")
            .insert_header(("Authorization", format!("Bearer {}", token())))
            .set_json(serde_json::json!({
                "artist_id": 1,
                "producer_id": 1,
                "merch_title": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }
}
