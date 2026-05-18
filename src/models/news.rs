use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::utils::url::validate_url;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct News {
    pub id: i64,
    pub url: Option<String>,
    pub title: Option<String>,
    pub vote_score: Option<i32>,
    pub flagged: Option<bool>,
    pub flagged_at: Option<DateTime<Utc>>,
    pub user_id: i64,
    pub image_url: Option<String>,
    pub active: Option<bool>,
    pub comments_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNewsRequest {
    pub url: String,
    pub title: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNewsRequest {
    pub active: Option<bool>,
    pub comments_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewsResponse {
    pub id: i64,
    pub url: Option<String>,
    pub title: Option<String>,
    pub vote_score: Option<i32>,
    pub flagged: Option<bool>,
    pub image_url: Option<String>,
    pub active: Option<bool>,
    pub comments_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl News {
    pub fn to_response(&self) -> NewsResponse {
        NewsResponse {
            id: self.id,
            url: self.url.clone(),
            title: self.title.clone(),
            vote_score: self.vote_score,
            flagged: self.flagged,
            image_url: self.image_url.clone(),
            active: self.active,
            comments_enabled: self.comments_enabled,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn validate_url(url: &str) -> bool {
        validate_url(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_url_valid() {
        assert!(News::validate_url("https://example.com/article"));
        assert!(News::validate_url("http://news.site.com/post"));
    }

    #[test]
    fn validate_url_invalid() {
        assert!(!News::validate_url(""));
        assert!(!News::validate_url("not-a-url"));
        assert!(!News::validate_url("http://localhost/admin"));
        assert!(!News::validate_url("http://127.0.0.1/secret"));
        assert!(!News::validate_url("http://169.254.169.254/metadata"));
    }
}
