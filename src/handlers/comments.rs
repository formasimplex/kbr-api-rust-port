//! Comment handlers
//!
//! Provides endpoints for viewing, creating, and replying to comments.
//! Comments are associated with a commentable entity (e.g., News). Reading
//! is public; creating requires customer role or above.
//!
//! # Endpoints
//!
//! | Function | Method | Route | Auth | Description |
//! |----------|--------|-------|------|-------------|
//! | `show` | GET | `/v1/comment/{id}` | public | Retrieve a single comment by ID |
//! | `index` | GET | `/v1/comments` | public | List comments, optionally filtered by commentable_id |
//! | `create` | POST | `/v1/comments` | customer+ | Create a new comment on a commentable entity |
//! | `create_reply` | POST | `/v1/comments/{parent_id}` | customer+ | Create a reply to an existing comment |

use actix_web::{web, HttpResponse};
use sqlx::FromRow;

use crate::app::AppState;
use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_customer_or_above;
use crate::error::AppError;
use crate::models::comment::{Comment, CommentResponse, CreateCommentRequest};

#[derive(Debug, FromRow)]
struct CommentRow {
    id: i64,
    content: Option<String>,
    flagged: Option<bool>,
    flagged_at: Option<chrono::NaiveDateTime>,
    commentable_type: String,
    commentable_id: i64,
    user_id: i64,
    parent_id: Option<i32>,
    created_at: chrono::NaiveDateTime,
    updated_at: chrono::NaiveDateTime,
}

impl From<CommentRow> for Comment {
    fn from(row: CommentRow) -> Self {
        Comment {
            id: row.id,
            content: row.content,
            flagged: row.flagged,
            flagged_at: row.flagged_at.map(|dt| dt.and_utc()),
            commentable_type: row.commentable_type,
            commentable_id: row.commentable_id,
            user_id: row.user_id,
            parent_id: row.parent_id,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

/// Retrieve a single comment by ID.
///
/// No authentication required.
///
/// # Response
///
/// `200 OK` — `CommentResponse`
/// `404 Not Found` — comment does not exist
pub async fn show(
    path: web::Path<i64>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let row = sqlx::query_as::<_, CommentRow>(
        r"SELECT id, content, flagged, flagged_at, commentable_type, commentable_id,
                  user_id, parent_id, created_at, updated_at
           FROM comments WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;

    match row {
        Some(r) => {
            let comment: Comment = r.into();
            Ok(HttpResponse::Ok().json(comment.to_response(None)))
        }
        None => Err(AppError::NotFound(format!("Comment #{}", id))),
    }
}

/// List comments.
///
/// Returns all comments ordered by creation date. Optionally filter by
/// `commentable_id` query parameter. No authentication required.
///
/// # Response
///
/// `200 OK` — JSON array of `CommentResponse`
pub async fn index(
    _user: Option<CurrentUser>,
    query: web::Query<serde_json::Value>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let commentable_id = query
        .get("commentable_id")
        .and_then(|v| v.get("commentable_id"))
        .and_then(|v| v.as_i64());

    let rows = if let Some(cid) = commentable_id {
        sqlx::query_as::<_, CommentRow>(
            r"SELECT id, content, flagged, flagged_at, commentable_type, commentable_id,
                      user_id, parent_id, created_at, updated_at
               FROM comments WHERE commentable_id = $1 ORDER BY created_at"
        )
        .bind(cid)
        .fetch_all(&state.db)
        .await?
    } else {
        sqlx::query_as::<_, CommentRow>(
            r"SELECT id, content, flagged, flagged_at, commentable_type, commentable_id,
                      user_id, parent_id, created_at, updated_at
               FROM comments ORDER BY created_at"
        )
        .fetch_all(&state.db)
        .await?
    };

    let comments: Vec<Comment> = rows.into_iter().map(|r| r.into()).collect();
    let responses: Vec<CommentResponse> = comments.iter().map(|c| c.to_response(None)).collect();
    Ok(HttpResponse::Ok().json(responses))
}

/// Create a new comment on a commentable entity.
///
/// Validates that content is non-empty and commentable_type is valid.
/// Requires customer role or above.
///
/// # Response
///
/// `201 Created` — `CommentResponse` for the new comment
/// `403 Forbidden` — insufficient role
/// `422 Unprocessable` — empty content or invalid commentable_type
pub async fn create(
    user: CurrentUser,
    body: web::Json<CreateCommentRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if !is_customer_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    if body.content.is_empty() {
        return Err(AppError::Validation("Comment content is required".to_string()));
    }
    if !Comment::valid_commentable_type(&body.commentable_type) {
        return Err(AppError::Validation(format!(
            "Invalid commentable_type: {}",
            body.commentable_type
        )));
    }

    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, CommentRow>(
        r"INSERT INTO comments (content, flagged, commentable_type, commentable_id,
                                user_id, parent_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, content, flagged, flagged_at, commentable_type, commentable_id,
                     user_id, parent_id, created_at, updated_at"
    )
    .bind(&body.content)
    .bind(false)
    .bind(&body.commentable_type)
    .bind(body.commentable_id)
    .bind(user.id)
    .bind(body.parent_id)
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let comment: Comment = row.into();
    Ok(HttpResponse::Created().json(comment.to_response(None)))
}

/// Create a reply to an existing comment.
///
/// Sets the parent_id to the comment identified by the path parameter.
/// Requires customer role or above.
///
/// # Response
///
/// `201 Created` — `CommentResponse` for the new reply
/// `403 Forbidden` — insufficient role
/// `422 Unprocessable` — empty content
pub async fn create_reply(
    user: CurrentUser,
    path: web::Path<i32>,
    body: web::Json<CreateCommentRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if !is_customer_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    if body.content.is_empty() {
        return Err(AppError::Validation("Comment content is required".to_string()));
    }

    let parent_id = path.into_inner();
    let now = chrono::Utc::now().naive_utc();

    let row = sqlx::query_as::<_, CommentRow>(
        r"INSERT INTO comments (content, flagged, commentable_type, commentable_id,
                                user_id, parent_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, content, flagged, flagged_at, commentable_type, commentable_id,
                     user_id, parent_id, created_at, updated_at"
    )
    .bind(&body.content)
    .bind(false)
    .bind(&body.commentable_type)
    .bind(body.commentable_id)
    .bind(user.id)
    .bind(Some(parent_id))
    .bind(now)
    .bind(now)
    .fetch_one(&state.db)
    .await?;

    let comment: Comment = row.into();
    Ok(HttpResponse::Created().json(comment.to_response(None)))
}

pub fn config_routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/comment/{id}", web::get().to(show))
        .route("/comments", web::get().to(index))
        .route("/comments", web::post().to(create))
        .route("/comments/{parent_id}", web::post().to(create_reply));
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
        crate::test_utils::build_test_state(pool).await
    }

    fn admin_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap()
    }

    fn customer_token() -> String {
        encode_token_with_role(1, TEST_SECRET, 3, Some("customer".to_string()), 1).unwrap()
    }

    async fn seed_comment(state: &AppState) -> i64 {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let content = format!("Test comment {}", suffix);

        sqlx::query_scalar::<_, i64>(
            r"INSERT INTO comments (content, flagged, commentable_type, commentable_id,
                                    user_id, created_at, updated_at)
               VALUES ($1, false, 'News', 1, 1, NOW(), NOW())
               RETURNING id"
        )
        .bind(&content)
        .fetch_one(&state.db)
        .await
        .expect("Failed to seed comment")
    }

    async fn cleanup_comment(state: &AppState, id: i64) {
        let _ = sqlx::query(&format!(r"DELETE FROM comments WHERE id = {}", id))
            .execute(&state.db)
            .await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comment_show_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = get_state().await;
        let comment_id = seed_comment(&state).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(&format!("/comment/{}", comment_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        cleanup_comment(&state, comment_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comment_show_not_found() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = get_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/comment/99999999")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comments_index_public() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = get_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/comments")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comment_create_authenticated() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = get_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/comments")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "content": "Nice!",
                "commentable_type": "News",
                "commentable_id": 1
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(status, 201, "Expected 201, got {}: {:?}", status, body);

        let id = body["id"].as_i64().expect("response should have id");
        cleanup_comment(&state, id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comment_create_empty_content() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = get_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/comments")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "content": "",
                "commentable_type": "News",
                "commentable_id": 1
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comment_create_invalid_commentable_type() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = get_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/comments")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "content": "Test",
                "commentable_type": "Invalid",
                "commentable_id": 1
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comment_create_reply() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = get_state().await;
        let parent_id = seed_comment(&state).await;

        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state.clone()))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri(&format!("/comments/{}", parent_id))
            .insert_header(("Authorization", format!("Bearer {}", customer_token())))
            .set_json(serde_json::json!({
                "content": "Reply content",
                "commentable_type": "News",
                "commentable_id": 1
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        let status = resp.status();
        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(status, 201, "Expected 201, got {}: {:?}", status, body);

        let reply_id = body["id"].as_i64().expect("response should have id");
        cleanup_comment(&state, reply_id).await;
        cleanup_comment(&state, parent_id).await;
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comment_create_reply_empty_content() {
        unsafe {
            std::env::set_var("DATABASE_URL", TEST_DB_URL);
            std::env::set_var("JWT_SECRET", TEST_SECRET);
        }
        let state = get_state().await;
        let app = test::init_service(
            App::new()
                .app_data(web::Data::new(state))
                .configure(config_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/comments/1")
            .insert_header(("Authorization", format!("Bearer {}", customer_token())))
            .set_json(serde_json::json!({
                "content": "",
                "commentable_type": "News",
                "commentable_id": 1
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 422);
    }
}
