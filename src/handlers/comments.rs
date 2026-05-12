use actix_web::{web, HttpResponse};

use crate::auth::middleware::CurrentUser;
use crate::auth::roles::is_customer_or_above;
use crate::error::AppError;
use crate::models::comment::{Comment, CommentResponse, CreateCommentRequest};

pub async fn show(path: web::Path<i64>) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    match mock_find_comment(id) {
        Some(c) => Ok(HttpResponse::Ok().json(c.to_response())),
        None => Err(AppError::NotFound(format!("Comment #{}", id))),
    }
}

pub async fn index(
    _user: Option<CurrentUser>,
    query: web::Query<serde_json::Value>,
) -> Result<HttpResponse, AppError> {
    let commentable_id = query
        .get("commentable_id")
        .and_then(|v| v.get("commentable_id"))
        .and_then(|v| v.as_i64());
    let comments = if let Some(cid) = commentable_id {
        mock_comments_for_commentable(cid)
    } else {
        mock_all_comments()
    };
    let responses: Vec<CommentResponse> = comments.iter().map(|c| c.to_response()).collect();
    Ok(HttpResponse::Ok().json(responses))
}

pub async fn create(
    user: CurrentUser,
    body: web::Json<CreateCommentRequest>,
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
    let comment = Comment {
        id: 999,
        content: Some(body.content.clone()),
        flagged: Some(false),
        flagged_at: None,
        commentable_type: body.commentable_type.clone(),
        commentable_id: body.commentable_id,
        user_id: user.id,
        parent_id: body.parent_id,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(comment.to_response()))
}

pub async fn create_reply(
    user: CurrentUser,
    path: web::Path<i32>,
    body: web::Json<CreateCommentRequest>,
) -> Result<HttpResponse, AppError> {
    if !is_customer_or_above(&user.role) {
        return Err(AppError::Forbidden("Not Authorized".to_string()));
    }
    if body.content.is_empty() {
        return Err(AppError::Validation("Comment content is required".to_string()));
    }
    let comment = Comment {
        id: 999,
        content: Some(body.content.clone()),
        flagged: Some(false),
        flagged_at: None,
        commentable_type: body.commentable_type.clone(),
        commentable_id: body.commentable_id,
        user_id: user.id,
        parent_id: Some(path.into_inner()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    Ok(HttpResponse::Created().json(comment.to_response()))
}

fn mock_all_comments() -> Vec<Comment> {
    vec![Comment {
        id: 1,
        content: Some("Great post!".to_string()),
        flagged: Some(false),
        flagged_at: None,
        commentable_type: "News".to_string(),
        commentable_id: 1,
        user_id: 1,
        parent_id: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }]
}

fn mock_comments_for_commentable(_id: i64) -> Vec<Comment> {
    mock_all_comments()
}

fn mock_find_comment(id: i64) -> Option<Comment> {
    if id == 1 {
        Some(Comment {
            id: 1,
            content: Some("Great post!".to_string()),
            flagged: Some(false),
            flagged_at: None,
            commentable_type: "News".to_string(),
            commentable_id: 1,
            user_id: 1,
            parent_id: None,
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
            .route("/comment/{id}", web::get().to(show))
            .route("/comments", web::get().to(index))
            .route("/comments", web::post().to(create))
            .route("/comments/{parent_id}", web::post().to(create_reply)),
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
    async fn comment_show_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/comment/1").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comment_show_not_found() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/comment/9999").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comments_index_public() {
        let app = test::init_service(App::new().configure(config_routes)).await;
        let req = test::TestRequest::get().uri("/v1/comments").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comment_create_authenticated() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/comments")
            .insert_header(("Authorization", format!("Bearer {}", admin_token())))
            .set_json(serde_json::json!({
                "content": "Nice!",
                "commentable_type": "News",
                "commentable_id": 1
            }))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 201);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn comment_create_empty_content() {
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/comments")
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
        unsafe { std::env::set_var("JWT_SECRET", TEST_SECRET); }
        let app = test::init_service(App::new().configure(config_routes)).await;

        let req = test::TestRequest::post()
            .uri("/v1/comments")
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
}
