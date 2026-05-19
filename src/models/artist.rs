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
    pub image_urls: Vec<String>,
    pub image_thumbnail_urls: Vec<String>,
}

impl Artist {
    pub fn to_response(&self, image_urls: Vec<String>, image_thumbnail_urls: Vec<String>) -> ArtistResponse {
        ArtistResponse {
            id: self.id,
            name: self.name.clone(),
            genre: self.genre.clone(),
            bio: self.bio.clone(),
            spotify_id: self.spotify_id.clone(),
            sub_heading: self.sub_heading.clone(),
            intro: self.intro.clone(),
            image_urls,
            image_thumbnail_urls,
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
