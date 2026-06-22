use actix_multipart::Multipart;
use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::error::AppError;
use crate::models::artist_link::ArtistLinkResponse;

const MAX_UPLOAD_SIZE: usize = 10 * 1024 * 1024; // 10MB

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

/// Parsed multipart result containing artist fields and uploaded images.
pub struct ParsedArtistUpdate {
    pub request: UpdateArtistRequest,
    pub image_files: Vec<(Vec<u8>, String, String)>,
}

impl UpdateArtistRequest {
    /// Parse multipart/form-data into artist fields and image uploads.
    ///
    /// Expects fields prefixed with `artist[` (e.g. `artist[name]`, `artist[bio]`).
    /// Image files are sent as `artist[images][]`.
    pub async fn from_multipart(mut payload: Multipart) -> Result<ParsedArtistUpdate, AppError> {
        let mut name: Option<String> = None;
        let mut genre: Option<String> = None;
        let mut bio: Option<String> = None;
        let mut spotify_id: Option<String> = None;
        let mut sub_heading: Option<String> = None;
        let mut intro: Option<String> = None;
        let mut image_files: Vec<(Vec<u8>, String, String)> = Vec::new();

        while let Some(item) = payload.next().await {
            let mut field = item.map_err(|e| AppError::Storage(format!("Multipart error: {}", e)))?;

            let field_name = field
                .name()
                .ok_or_else(|| AppError::Validation("Field name is required".to_string()))?
                .to_string();

            let data = field
                .bytes(MAX_UPLOAD_SIZE)
                .await
                .map_err(|e| AppError::Storage(format!("Failed to read field: {}", e)))?
                .map_err(|e| AppError::Storage(format!("Failed to read field: {}", e)))?
                .to_vec();

            match field_name.as_str() {
                "artist[name]" => {
                    name = Some(
                        String::from_utf8(data)
                            .map_err(|_| AppError::Validation("Invalid name".to_string()))?
                            .trim()
                            .to_string(),
                    );
                }
                "artist[genre]" => {
                    genre = Some(
                        String::from_utf8(data)
                            .map_err(|_| AppError::Validation("Invalid genre".to_string()))?
                            .trim()
                            .to_string(),
                    );
                }
                "artist[bio]" => {
                    bio = Some(
                        String::from_utf8(data)
                            .map_err(|_| AppError::Validation("Invalid bio".to_string()))?
                            .trim()
                            .to_string(),
                    );
                }
                "artist[spotifyId]" => {
                    spotify_id = Some(
                        String::from_utf8(data)
                            .map_err(|_| AppError::Validation("Invalid spotifyId".to_string()))?
                            .trim()
                            .to_string(),
                    );
                }
                "artist[subHeading]" => {
                    sub_heading = Some(
                        String::from_utf8(data)
                            .map_err(|_| AppError::Validation("Invalid subHeading".to_string()))?
                            .trim()
                            .to_string(),
                    );
                }
                "artist[intro]" => {
                    intro = Some(
                        String::from_utf8(data)
                            .map_err(|_| AppError::Validation("Invalid intro".to_string()))?
                            .trim()
                            .to_string(),
                    );
                }
                "artist[images][]" => {
                    let ct = field.content_type().ok_or_else(|| {
                        AppError::Validation("Content type is required for images".to_string())
                    })?;

                    if !ct.to_string().starts_with("image/") {
                        return Err(AppError::Validation(
                            "Only image files are allowed".to_string(),
                        ));
                    }

                    let filename = field
                        .content_disposition()
                        .and_then(|cd| cd.get_filename().map(|n| n.to_string()))
                        .unwrap_or_else(|| "upload".to_string());

                    image_files.push((data, filename, ct.to_string()));
                }
                _ => {}
            }
        }

        Ok(ParsedArtistUpdate {
            request: UpdateArtistRequest {
                name,
                genre,
                bio,
                spotify_id,
                sub_heading,
                intro,
            },
            image_files,
        })
    }
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
    pub artist_links: Vec<ArtistLinkResponse>,
}

impl Artist {
    pub fn to_response(
        &self,
        image_urls: Vec<String>,
        image_thumbnail_urls: Vec<String>,
        artist_links: Vec<ArtistLinkResponse>,
    ) -> ArtistResponse {
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
            artist_links,
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
