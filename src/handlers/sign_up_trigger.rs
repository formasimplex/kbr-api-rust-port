use actix_web::{web, HttpResponse};
use sqlx::FromRow;

use crate::app::AppState;
use crate::error::AppError;
use crate::models::sign_up_trigger::SignUpTrigger;
use crate::models::user::User;

#[derive(Debug, FromRow)]
struct SignUpTriggerRow {
    id: i64,
    email: Option<String>,
    token: Option<String>,
    expires_at: Option<String>,
    role: Option<String>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<SignUpTriggerRow> for SignUpTrigger {
    fn from(row: SignUpTriggerRow) -> Self {
        SignUpTrigger {
            id: row.id,
            email: row.email,
            token: row.token,
            expires_at: row.expires_at,
            role: row.role,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

pub async fn create(
    state: web::Data<AppState>,
    body: web::Json<crate::models::sign_up_trigger::CreateSignUpTriggerRequest>,
) -> Result<HttpResponse, AppError> {
    if !User::validate_email(&body.email) {
        return Err(AppError::Validation("Invalid email".to_string()));
    }

    let token = SignUpTrigger::generate_token();
    let expires_at = SignUpTrigger::generate_expires_at();
    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, SignUpTriggerRow>(
        r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6)
           RETURNING id, email, token, expires_at, role, created_at, updated_at"
    )
    .bind(&body.email)
    .bind(&token)
    .bind(&expires_at)
    .bind(&body.role)
    .bind(&now)
    .bind(&now)
    .fetch_one(&state.db)
    .await?;

    let trigger: SignUpTrigger = row.into();
    Ok(HttpResponse::Created().json(trigger.to_response()))
}

pub async fn show(
    state: web::Data<AppState>,
    path: web::Path<String>,
) -> Result<HttpResponse, AppError> {
    let token = path.into_inner();

    match sqlx::query_as::<_, SignUpTriggerRow>(
        r"SELECT id, email, token, expires_at, role, created_at, updated_at
           FROM sign_up_triggers WHERE token = $1"
    )
    .bind(&token)
    .fetch_optional(&state.db)
    .await?
    {
        Some(row) => {
            let trigger: SignUpTrigger = row.into();
            Ok(HttpResponse::Ok().json(trigger.to_response()))
        }
        None => Err(AppError::NotFound(format!("Sign-up trigger not found"))),
    }
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/v1")
            .route("/sign_up_trigger", web::post().to(create))
            .route("/sign_up_trigger/{token}", web::get().to(show)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    const TEST_DB_URL: &str = "postgresql://ws@localhost:5432/kbr_test";

    async fn get_state() -> AppState {
        AppState {
            db: sqlx::PgPool::connect(TEST_DB_URL)
                .await
                .expect("Failed to connect to test database"),
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_create_success() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state.clone())
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/sign_up_trigger")
            .set_json(serde_json::json!({
                "email": "invited@example.com",
                "role": "user"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body["token"].is_string());
        assert_eq!(body["email"], "invited@example.com");

        let _ = sqlx::query(r"DELETE FROM sign_up_triggers WHERE email = 'invited@example.com'")
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_create_invalid_email() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/v1/sign_up_trigger")
            .set_json(serde_json::json!({
                "email": "bad-email"
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_show_valid() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);

        let token = format!("rust_test_{}", chrono::Utc::now().timestamp_micros());
        let seed = sqlx::query_as::<_, SignUpTriggerRow>(
            r"INSERT INTO sign_up_triggers (email, token, expires_at, role, created_at, updated_at)
               VALUES ('showtest@example.com', $1, 'Dec 31, 2026 11:59 PM', 'user', NOW(), NOW())
               RETURNING id, email, token, expires_at, role, created_at, updated_at"
        )
        .bind(&token)
        .fetch_one(&state.db)
        .await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(get_state().await))
                .configure(config_routes),
        )
        .await;

        if let Ok(_row) = seed {
            let req = test::TestRequest::get()
                .uri(&format!("/v1/sign_up_trigger/{}", token))
                .to_request();
            let resp = test::call_service(&app, req).await;
            assert_eq!(resp.status(), 200);

            let body: serde_json::Value = test::read_body_json(resp).await;
            assert_eq!(body["email"], "showtest@example.com");

            let _ = sqlx::query(r"DELETE FROM sign_up_triggers WHERE email = 'showtest@example.com'")
                .execute(&state.db)
                .await;
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn sign_up_trigger_show_not_found() {
        unsafe { std::env::set_var("DATABASE_URL", TEST_DB_URL); }
        let state = web::Data::new(get_state().await);
        let app = test::init_service(
            App::new()
                .app_data(state)
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/v1/sign_up_trigger/nonexistent-token-xyz")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }
}
