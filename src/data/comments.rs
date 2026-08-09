use crate::models::comment::CommentWithUser;

const COMMENT_COLUMNS: &str = r#"c.id, c.content, c.flagged, c.flagged_at, c.commentable_type, c.commentable_id,
    c.user_id, c.parent_id, c.created_at, c.updated_at, u.username"#;

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<CommentWithUser>, sqlx::Error> {
    sqlx::query_as::<_, CommentWithUser>(
        &format!(r#"SELECT {} FROM comments c LEFT JOIN users u ON u.id = c.user_id WHERE c.id = $1"#, COMMENT_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list(pool: &sqlx::PgPool, commentable_id: Option<i64>) -> Result<Vec<CommentWithUser>, sqlx::Error> {
    if let Some(cid) = commentable_id {
        sqlx::query_as::<_, CommentWithUser>(
            &format!(r#"SELECT {} FROM comments c LEFT JOIN users u ON u.id = c.user_id WHERE c.commentable_id = $1 ORDER BY c.created_at"#, COMMENT_COLUMNS),
        )
        .bind(cid)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, CommentWithUser>(
            &format!(r#"SELECT {} FROM comments c LEFT JOIN users u ON u.id = c.user_id ORDER BY c.created_at"#, COMMENT_COLUMNS),
        )
        .fetch_all(pool)
        .await
    }
}

pub async fn create(
    pool: &sqlx::PgPool,
    content: &str,
    flagged: bool,
    commentable_type: &str,
    commentable_id: i64,
    user_id: i64,
    parent_id: Option<i32>,
    now: chrono::NaiveDateTime,
) -> Result<CommentWithUser, sqlx::Error> {
    sqlx::query_as::<_, CommentWithUser>(
        &format!(
            r#"WITH inserted AS (
               INSERT INTO comments (content, flagged, commentable_type, commentable_id,
                                     user_id, parent_id, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING id, content, flagged, flagged_at, commentable_type, commentable_id,
                         user_id, parent_id, created_at, updated_at
           )
           SELECT i.id, i.content, i.flagged, i.flagged_at, i.commentable_type, i.commentable_id,
                  i.user_id, i.parent_id, i.created_at, i.updated_at, u.username
           FROM inserted i LEFT JOIN users u ON u.id = i.user_id"#
        ),
    )
    .bind(content)
    .bind(flagged)
    .bind(commentable_type)
    .bind(commentable_id)
    .bind(user_id)
    .bind(parent_id)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}
