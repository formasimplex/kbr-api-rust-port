use sqlx::FromRow;

use crate::models::comment::Comment;

#[derive(Debug, FromRow)]
pub struct CommentRow {
    pub id: i64,
    pub content: Option<String>,
    pub flagged: Option<bool>,
    pub flagged_at: Option<chrono::NaiveDateTime>,
    pub commentable_type: String,
    pub commentable_id: i64,
    pub user_id: i64,
    pub parent_id: Option<i32>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
    pub username: Option<String>,
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

const COMMENT_COLUMNS: &str = r#"c.id, c.content, c.flagged, c.flagged_at, c.commentable_type, c.commentable_id,
    c.user_id, c.parent_id, c.created_at, c.updated_at, u.username"#;

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<CommentRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, CommentRow>(
        &format!(r#"SELECT {} FROM comments c LEFT JOIN users u ON u.id = c.user_id WHERE c.id = $1"#, COMMENT_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn list(pool: &sqlx::PgPool, commentable_id: Option<i64>) -> Result<Vec<CommentRow>, sqlx::Error> {
    let rows = if let Some(cid) = commentable_id {
        sqlx::query_as::<_, CommentRow>(
            &format!(r#"SELECT {} FROM comments c LEFT JOIN users u ON u.id = c.user_id WHERE c.commentable_id = $1 ORDER BY c.created_at"#, COMMENT_COLUMNS),
        )
        .bind(cid)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as::<_, CommentRow>(
            &format!(r#"SELECT {} FROM comments c LEFT JOIN users u ON u.id = c.user_id ORDER BY c.created_at"#, COMMENT_COLUMNS),
        )
        .fetch_all(pool)
        .await?
    };

    Ok(rows)
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
) -> Result<CommentRow, sqlx::Error> {
    let row = sqlx::query_as::<_, CommentRow>(
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
    .await?;

    Ok(row)
}
