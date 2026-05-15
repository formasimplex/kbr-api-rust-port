use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct UsersNews {
    pub id: i64,
    pub user_id: i64,
    pub news_id: i64,
    pub playlist_id: i64,
    pub position: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserNewsRequest {
    pub news_id: i64,
    pub playlist_id: i64,
    pub position: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UsersNewsResponse {
    pub id: i64,
    pub user_id: i64,
    pub news_id: i64,
    pub playlist_id: i64,
    pub position: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl UsersNews {
    pub fn to_response(&self) -> UsersNewsResponse {
        UsersNewsResponse {
            id: self.id,
            user_id: self.user_id,
            news_id: self.news_id,
            playlist_id: self.playlist_id,
            position: self.position,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}


