use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Comment {
    pub id: i64,
    pub content: Option<String>,
    pub flagged: Option<bool>,
    pub flagged_at: Option<NaiveDateTime>,
    pub commentable_type: String,
    pub commentable_id: i64,
    pub user_id: i64,
    pub parent_id: Option<i32>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Comment with joined users.username.
#[derive(Debug, FromRow)]
pub struct CommentWithUser {
    pub id: i64,
    pub content: Option<String>,
    pub flagged: Option<bool>,
    pub flagged_at: Option<NaiveDateTime>,
    pub commentable_type: String,
    pub commentable_id: i64,
    pub user_id: i64,
    pub parent_id: Option<i32>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub username: Option<String>,
}

impl From<CommentWithUser> for Comment {
    fn from(row: CommentWithUser) -> Self {
        Comment {
            id: row.id,
            content: row.content,
            flagged: row.flagged,
            flagged_at: row.flagged_at,
            commentable_type: row.commentable_type,
            commentable_id: row.commentable_id,
            user_id: row.user_id,
            parent_id: row.parent_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCommentRequest {
    pub content: String,
    pub commentable_type: String,
    pub commentable_id: i64,
    pub parent_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommentUser {
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommentResponse {
    pub id: i64,
    pub content: Option<String>,
    pub created_at: DateTime<Utc>,
    pub user: Option<CommentUser>,
    pub replies: Vec<CommentResponse>,
}

impl Comment {
    pub fn to_response(&self, username: Option<String>, replies: Vec<CommentResponse>) -> CommentResponse {
        CommentResponse {
            id: self.id,
            content: self.content.clone(),
            created_at: self.created_at.and_utc(),
            user: username.map(|u| CommentUser { username: Some(u) }),
            replies,
        }
    }

    pub fn is_reply(&self) -> bool {
        self.parent_id.is_some()
    }

    pub fn valid_commentable_type(ty: &str) -> bool {
        matches!(ty, "News" | "Campaign" | "KBREvent")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_is_reply() {
        let reply = Comment {
            id: 2,
            content: Some("Reply".to_string()),
            flagged: None,
            flagged_at: None,
            commentable_type: "News".to_string(),
            commentable_id: 5,
            user_id: 10,
            parent_id: Some(1),
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
        };
        assert!(reply.is_reply());

        let top = Comment {
            parent_id: None,
            ..reply.clone()
        };
        assert!(!top.is_reply());
    }

    #[test]
    fn valid_commentable_type() {
        assert!(Comment::valid_commentable_type("News"));
        assert!(Comment::valid_commentable_type("Campaign"));
        assert!(Comment::valid_commentable_type("KBREvent"));
        assert!(!Comment::valid_commentable_type("Album"));
        assert!(!Comment::valid_commentable_type(""));
    }
}
