use chrono::{NaiveDateTime};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Song {
    pub id: i64,
    pub name: Option<String>,
    pub duration: Option<String>,
    pub album_id: i64,
    pub artist_id: i64,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSongRequest {
    pub name: String,
    pub duration: Option<String>,
    pub album_id: i64,
    pub artist_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSongRequest {
    pub name: Option<String>,
    pub duration: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SongResponse {
    pub id: i64,
    pub name: Option<String>,
    pub duration: Option<String>,
    pub album_id: i64,
    pub artist_id: i64,
}

impl Song {
    pub fn to_response(&self) -> SongResponse {
        SongResponse {
            id: self.id,
            name: self.name.clone(),
            duration: self.duration.clone(),
            album_id: self.album_id,
            artist_id: self.artist_id,
        }
    }
}


