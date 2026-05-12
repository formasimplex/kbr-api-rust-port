use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Song {
    pub id: i64,
    pub name: Option<String>,
    pub duration: Option<String>,
    pub album_id: i64,
    pub artist_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn song_to_response() {
        let song = Song {
            id: 1,
            name: Some("Hit Song".to_string()),
            duration: Some("3:45".to_string()),
            album_id: 5,
            artist_id: 3,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = song.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.name, Some("Hit Song".to_string()));
        assert_eq!(resp.duration, Some("3:45".to_string()));
    }
}
