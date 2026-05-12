use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Comment {
    pub id: i64,
    pub content: Option<String>,
    pub flagged: Option<bool>,
    pub flagged_at: Option<DateTime<Utc>>,
    pub commentable_type: String,
    pub commentable_id: i64,
    pub user_id: i64,
    pub parent_id: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCommentRequest {
    pub content: String,
    pub commentable_type: String,
    pub commentable_id: i64,
    pub parent_id: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommentResponse {
    pub id: i64,
    pub content: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Comment {
    pub fn to_response(&self) -> CommentResponse {
        CommentResponse {
            id: self.id,
            content: self.content.clone(),
            created_at: self.created_at,
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
    fn comment_to_response() {
        let comment = Comment {
            id: 1,
            content: Some("Great post!".to_string()),
            flagged: Some(false),
            flagged_at: None,
            commentable_type: "News".to_string(),
            commentable_id: 5,
            user_id: 10,
            parent_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = comment.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.content, Some("Great post!".to_string()));
    }

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
            created_at: Utc::now(),
            updated_at: Utc::now(),
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
