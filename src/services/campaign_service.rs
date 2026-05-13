use crate::error::AppError;
use crate::models::campaign::{CreateCampaignRequest, SongEntry, ArtistLinkEntry};
use crate::models::artist_link::ArtistLink;

pub struct CampaignService;

impl CampaignService {
    pub fn validate_create_request(req: &CreateCampaignRequest) -> Result<(), AppError> {
        if req.name.is_empty() {
            return Err(AppError::Validation(
                "Campaign name is required".to_string(),
            ));
        }
        if let Some(count) = req.vinyl_sold_count
            && !crate::models::campaign::Campaign::validate_vinyl_sold_count(count) {
                return Err(AppError::Validation(
                    "vinyl_sold_count must be between 0 and 100".to_string(),
                ));
            }
        Ok(())
    }

    pub fn parse_track_list(json_strings: &[String]) -> Result<Vec<SongEntry>, AppError> {
        let mut tracks = Vec::new();
        for s in json_strings {
            let entry: SongEntry = serde_json::from_str(s)
                .map_err(|e| AppError::Validation(format!("Invalid track JSON: {}", e)))?;
            if entry.name.is_empty() {
                return Err(AppError::Validation(
                    "Track name is required".to_string(),
                ));
            }
            tracks.push(entry);
        }
        Ok(tracks)
    }

    pub fn parse_artist_links(json_strings: &[String]) -> Result<Vec<ArtistLinkEntry>, AppError> {
        let mut links = Vec::new();
        for s in json_strings {
            let entry: ArtistLinkEntry = serde_json::from_str(s)
                .map_err(|e| AppError::Validation(format!("Invalid link JSON: {}", e)))?;
            if !ArtistLink::validate_url(&entry.url) {
                return Err(AppError::Validation(format!(
                    "Invalid URL for link type {}: {}",
                    entry.link_type, entry.url
                )));
            }
            links.push(entry);
        }
        Ok(links)
    }

    pub fn validate_page_type(page_type: i32) -> Result<(), AppError> {
        match page_type {
            0..=2 => Ok(()),
            _ => Err(AppError::Validation(format!(
                "Invalid page_type: {}. Must be 0 (minimalist), 1 (social_butterfly), or 2 (story_teller)",
                page_type
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_create_request_valid() {
        let req = CreateCampaignRequest {
            artist_id: 1,
            name: "Test Campaign".to_string(),
            vinyl_sold_count: Some(50),
            track_list: vec![],
            artist_links: vec![],
            layout_style: None,
            description: None,
        };
        assert!(CampaignService::validate_create_request(&req).is_ok());
    }

    #[test]
    fn validate_create_request_empty_name() {
        let req = CreateCampaignRequest {
            artist_id: 1,
            name: "".to_string(),
            vinyl_sold_count: None,
            track_list: vec![],
            artist_links: vec![],
            layout_style: None,
            description: None,
        };
        let err = CampaignService::validate_create_request(&req).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_create_request_vinyl_count_too_high() {
        let req = CreateCampaignRequest {
            artist_id: 1,
            name: "Test".to_string(),
            vinyl_sold_count: Some(101),
            track_list: vec![],
            artist_links: vec![],
            layout_style: None,
            description: None,
        };
        let err = CampaignService::validate_create_request(&req).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn parse_track_list_valid() {
        let jsons = vec![
            serde_json::to_string(&SongEntry {
                name: "Track 1".to_string(),
                duration: Some("3:30".to_string()),
            })
            .unwrap(),
        ];
        let tracks = CampaignService::parse_track_list(&jsons).unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].name, "Track 1");
    }

    #[test]
    fn parse_track_list_empty_name() {
        let jsons = vec![serde_json::to_string(&SongEntry {
            name: "".to_string(),
            duration: None,
        })
        .unwrap()];
        let err = CampaignService::parse_track_list(&jsons).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn parse_track_list_invalid_json() {
        let jsons = vec!["not valid json".to_string()];
        let err = CampaignService::parse_track_list(&jsons).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn parse_artist_links_valid() {
        let jsons = vec![serde_json::to_string(&ArtistLinkEntry {
            link_type: 1,
            url: "https://spotify.com/artist/123".to_string(),
        })
        .unwrap()];
        let links = CampaignService::parse_artist_links(&jsons).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].link_type, 1);
    }

    #[test]
    fn parse_artist_links_invalid_url() {
        let jsons = vec![serde_json::to_string(&ArtistLinkEntry {
            link_type: 1,
            url: "not-a-url".to_string(),
        })
        .unwrap()];
        let err = CampaignService::parse_artist_links(&jsons).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn validate_page_type_valid() {
        assert!(CampaignService::validate_page_type(0).is_ok());
        assert!(CampaignService::validate_page_type(1).is_ok());
        assert!(CampaignService::validate_page_type(2).is_ok());
    }

    #[test]
    fn validate_page_type_invalid() {
        let err = CampaignService::validate_page_type(3).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
