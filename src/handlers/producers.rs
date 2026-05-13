use actix_web::{web, HttpResponse};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::error::AppError;
use crate::models::producer::{CreateProducerRequest, Producer, ProducerResponse, UpdateProducerRequest};

#[derive(Debug, FromRow)]
struct ProducerRow {
    id: i64,
    description: Option<String>,
    producer_name: String,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<ProducerRow> for Producer {
    fn from(row: ProducerRow) -> Self {
        Producer {
            id: row.id,
            description: row.description,
            producer_name: row.producer_name,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

pub async fn index(
    _user: CurrentUser,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let rows = sqlx::query_as::<_, ProducerRow>(
        r"SELECT id, description, producer_name, created_at, updated_at FROM producers ORDER BY id"
    )
    .fetch_all(&state.db)
    .await?;

    let producers: Vec<Producer> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<ProducerResponse> = producers.iter().map(|p| p.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn create(
    state: web::Data<AppState>,
    _user: CurrentUser,
    body: web::Json<CreateProducerRequest>,
) -> Result<HttpResponse, AppError> {
    if body.producer_name.is_empty() {
        return Err(AppError::Validation("producer_name is required".to_string()));
    }

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, ProducerRow>(
        r"INSERT INTO producers (producer_name, description, created_at, updated_at)
           VALUES ($1, $2, $3, $4)
           RETURNING id, description, producer_name, created_at, updated_at"
    )
    .bind(&body.producer_name)
    .bind(&body.description)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let producer: Producer = row.into();
    Ok(HttpResponse::Created().json(producer.to_response()))
}

pub async fn update(
    state: web::Data<AppState>,
    _user: CurrentUser,
    path: web::Path<i64>,
    body: web::Json<UpdateProducerRequest>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();

    let name = body.producer_name.clone();
    let desc = body.description.clone();

    if let Some(ref n) = body.producer_name
        && n.is_empty() {
            return Err(AppError::Validation("producer_name cannot be empty".to_string()));
        }

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, ProducerRow>(
        r"UPDATE producers
           SET producer_name = COALESCE($1, producer_name),
               description = COALESCE($2, description),
               updated_at = $3
           WHERE id = $4
           RETURNING id, description, producer_name, created_at, updated_at"
    )
    .bind(name.as_deref())
    .bind(desc.as_deref())
    .bind(now)
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(r) => {
            let producer: Producer = r.into();
            Ok(HttpResponse::Ok().json(producer.to_response()))
        }
        None => Err(AppError::NotFound(format!("Producer #{}", id))),
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/producers", web::get().to(index))
            .route("/producers", web::post().to(create))
            .route("/producers/{id}", web::put().to(update)),
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

    fn admin_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string())).unwrap()
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producers_index_authenticated() {
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
            .uri("/v1/producers")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producer_create_success() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let name = format!("New Pressing Co {}", suffix);

        let req = test::TestRequest::post()
            .uri("/v1/producers")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "producer_name": name,
                "description": "Premium vinyl"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(status, 201, "Expected 201, got {}: {:?}", status, body);

        let _ = sqlx::query(&format!(r"DELETE FROM producers WHERE producer_name = '{}'", name))
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producer_create_empty_name() {
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

        let req = test::TestRequest::post()
            .uri("/v1/producers")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "producer_name": ""
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producer_update_success() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = web::Data::new(get_state().await);

        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let name = format!("Update Test {}", suffix);
        let new_name = format!("Updated {}", suffix);

        let id: i64 = sqlx::query_scalar::<_, i64>(
            r"INSERT INTO producers (producer_name, description, created_at, updated_at)
               VALUES ($1, 'Original desc', NOW(), NOW())
               RETURNING id"
        )
        .bind(&name)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed producer");

        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::put()
            .uri(&format!("/v1/producers/{}", id))
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "producer_name": new_name,
                "description": "Updated desc"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["producer_name"], new_name);
        assert_eq!(body["description"], "Updated desc");

        let _ = sqlx::query(&format!(r"DELETE FROM producers WHERE id = {}", id))
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn producer_update_not_found() {
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

        let req = test::TestRequest::put()
            .uri("/v1/producers/99999999")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "producer_name": "Updated"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }
}
