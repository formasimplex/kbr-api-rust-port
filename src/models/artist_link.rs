use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct ArtistLink {
    pub id: i64,
    pub artist_id: i64,
    pub link_type: i32,
    pub url: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateArtistLinkRequest {
    pub artist_id: i64,
    pub link_type: i32,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtistLinkResponse {
    pub id: i64,
    pub artist_id: i64,
    pub link_type: i32,
    pub url: String,
}

pub const LINK_TYPES: &[(&str, i32)] = &[
    ("soundcloud", 0),
    ("spotify", 1),
    ("youtube", 2),
    ("instagram", 3),
    ("facebook", 4),
    ("twitter", 5),
    ("tiktok", 6),
    ("bandcamp", 7),
    ("apple_music", 8),
    ("beatport", 9),
    ("website", 10),
];

impl ArtistLink {
    pub fn to_response(&self) -> ArtistLinkResponse {
        ArtistLinkResponse {
            id: self.id,
            artist_id: self.artist_id,
            link_type: self.link_type,
            url: self.url.clone(),
        }
    }

    pub fn validate_url(url: &str) -> bool {
        !url.is_empty() && url.len() <= 255 && (url.starts_with("http://") || url.starts_with("https://"))
    }

    pub fn link_type_name(link_type: i32) -> Option<&'static str> {
        LINK_TYPES.iter().find(|(_, t)| *t == link_type).map(|(name, _)| *name)
    }

    pub fn available_link_types() -> Vec<serde_json::Value> {
        LINK_TYPES
            .iter()
            .map(|(name, id)| {
                serde_json::json!({ "name": name, "id": id })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artist_link_to_response() {
        let link = ArtistLink {
            id: 1,
            artist_id: 5,
            link_type: 1,
            url: "https://spotify.com/artist/123".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = link.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.link_type, 1);
    }

    #[test]
    fn validate_url_valid() {
        assert!(ArtistLink::validate_url("https://example.com"));
        assert!(ArtistLink::validate_url("http://example.com"));
    }

    #[test]
    fn validate_url_invalid() {
        assert!(!ArtistLink::validate_url(""));
        assert!(!ArtistLink::validate_url("not-a-url"));
        assert!(!ArtistLink::validate_url(&"a".repeat(256)));
    }

    #[test]
    fn link_type_name() {
        assert_eq!(ArtistLink::link_type_name(0), Some("soundcloud"));
        assert_eq!(ArtistLink::link_type_name(1), Some("spotify"));
        assert_eq!(ArtistLink::link_type_name(10), Some("website"));
        assert_eq!(ArtistLink::link_type_name(99), None);
    }

    #[test]
    fn available_link_types_returns_all() {
        let types = ArtistLink::available_link_types();
        assert_eq!(types.len(), LINK_TYPES.len());
    }
}
