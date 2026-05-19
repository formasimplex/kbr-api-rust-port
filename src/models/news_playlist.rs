use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct NewsPlaylist {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateNewsPlaylistRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateNewsPlaylistRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NewsPlaylistResponse {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl NewsPlaylist {
    pub fn to_response(&self) -> NewsPlaylistResponse {
        NewsPlaylistResponse {
            id: self.id,
            user_id: self.user_id,
            name: self.name.clone(),
            description: self.description.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}


