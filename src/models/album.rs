use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Album {
    pub id: i64,
    pub name: Option<String>,
    pub release_date: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAlbumRequest {
    pub name: String,
    pub release_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateAlbumRequest {
    pub name: Option<String>,
    pub release_date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlbumResponse {
    pub id: i64,
    pub name: Option<String>,
    pub release_date: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Album {
    pub fn to_response(&self) -> AlbumResponse {
        AlbumResponse {
            id: self.id,
            name: self.name.clone(),
            release_date: self.release_date.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}


