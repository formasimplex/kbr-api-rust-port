use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Album {
    pub id: i64,
    pub name: Option<String>,
    pub release_date: Option<NaiveDate>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
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
            release_date: self.release_date.map(|d| d.format("%Y-%m-%d").to_string()),
            created_at: self.created_at.and_utc(),
            updated_at: self.updated_at.and_utc(),
        }
    }
}


