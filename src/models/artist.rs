use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Artist {
    pub id: i64,
    pub name: Option<String>,
    pub genre: Option<String>,
    pub bio: Option<String>,
    pub user_id: Option<i64>,
    pub prospect: Option<bool>,
    pub spotify_id: Option<String>,
    pub sub_heading: Option<String>,
    pub intro: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateArtistRequest {
    pub name: String,
    pub genre: Option<String>,
    pub bio: Option<String>,
    pub prospect: Option<bool>,
    pub spotify_id: Option<String>,
    pub sub_heading: Option<String>,
    pub intro: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateArtistRequest {
    pub name: Option<String>,
    pub genre: Option<String>,
    pub bio: Option<String>,
    pub spotify_id: Option<String>,
    pub sub_heading: Option<String>,
    pub intro: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtistResponse {
    pub id: i64,
    pub name: Option<String>,
    pub genre: Option<String>,
    pub bio: Option<String>,
    pub spotify_id: Option<String>,
    pub sub_heading: Option<String>,
    pub intro: Option<String>,
}

impl Artist {
    pub fn to_response(&self) -> ArtistResponse {
        ArtistResponse {
            id: self.id,
            name: self.name.clone(),
            genre: self.genre.clone(),
            bio: self.bio.clone(),
            spotify_id: self.spotify_id.clone(),
            sub_heading: self.sub_heading.clone(),
            intro: self.intro.clone(),
        }
    }

    pub fn validate_intro(intro: &str) -> bool {
        intro.len() <= 300
    }

    pub fn validate_bio(bio: &str) -> bool {
        bio.len() <= 3000
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artist_to_response() {
        let artist = Artist {
            id: 1,
            name: Some("DJ Test".to_string()),
            genre: Some("Electronic".to_string()),
            bio: Some("A great DJ".to_string()),
            user_id: Some(10),
            prospect: Some(false),
            spotify_id: Some("spotify-123".to_string()),
            sub_heading: Some("Sub heading".to_string()),
            intro: Some("Intro text".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = artist.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.name, Some("DJ Test".to_string()));
        assert_eq!(resp.genre, Some("Electronic".to_string()));
    }

    #[test]
    fn validate_intro_valid() {
        assert!(Artist::validate_intro("Short intro"));
        assert!(Artist::validate_intro(&"a".repeat(300)));
    }

    #[test]
    fn validate_intro_too_long() {
        assert!(!Artist::validate_intro(&"a".repeat(301)));
    }

    #[test]
    fn validate_bio_valid() {
        assert!(Artist::validate_bio("Short bio"));
        assert!(Artist::validate_bio(&"a".repeat(3000)));
    }

    #[test]
    fn validate_bio_too_long() {
        assert!(!Artist::validate_bio(&"a".repeat(3001)));
    }
}
