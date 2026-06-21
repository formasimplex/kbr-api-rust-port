use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Campaign {
    pub id: i64,
    pub artist_id: i64,
    pub name: Option<String>,
    pub active: Option<bool>,
    pub vinyl_sold_count: Option<i32>,
    pub campaign_start_date: Option<DateTime<Utc>>,
    pub campaign_end_date: Option<DateTime<Utc>>,
    pub progress: Option<i32>,
    pub album_id: Option<i64>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCampaignRequest {
    pub artist_id: i64,
    pub name: String,
    pub vinyl_sold_count: Option<i32>,
    #[serde(default)]
    pub track_list: Vec<SongEntry>,
    #[serde(default)]
    pub artist_links: Vec<ArtistLinkEntry>,
    pub layout_style: Option<i32>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SongEntry {
    pub name: String,
    pub duration: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistLinkEntry {
    pub link_type: i32,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCampaignRequest {
    pub name: Option<String>,
    pub active: Option<bool>,
    pub vinyl_sold_count: Option<i32>,
    pub progress: Option<i32>,
    pub campaign_start_date: Option<DateTime<Utc>>,
    pub campaign_end_date: Option<DateTime<Utc>>,
}

/// Request body for campaign activation.
///
/// Contains the campaign ID to activate and the desired start date.
/// The end date is computed as `start_date + CAMPAIGN_DURATION_DAYS`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivateCampaignRequest {
    pub campaign_id: i64,
    pub start_date: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CampaignResponse {
    pub id: i64,
    pub artist_id: i64,
    pub name: Option<String>,
    pub active: Option<bool>,
    pub vinyl_sold_count: Option<i32>,
    pub campaign_start_date: Option<DateTime<Utc>>,
    pub campaign_end_date: Option<DateTime<Utc>>,
    pub progress: Option<i32>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Campaign {
    pub fn to_response(&self) -> CampaignResponse {
        CampaignResponse {
            id: self.id,
            artist_id: self.artist_id,
            name: self.name.clone(),
            active: self.active,
            vinyl_sold_count: self.vinyl_sold_count,
            campaign_start_date: self.campaign_start_date,
            campaign_end_date: self.campaign_end_date,
            progress: self.progress,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    pub fn validate_vinyl_sold_count(count: i32) -> bool {
        (0..=crate::constants::VINYL_TARGET).contains(&count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_is_deleted() {
        let deleted = Campaign {
            id: 1,
            artist_id: 1,
            name: None,
            active: None,
            vinyl_sold_count: None,
            campaign_start_date: None,
            campaign_end_date: None,
            progress: None,
            album_id: None,
            deleted_at: Some(Utc::now()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(deleted.is_deleted());

        let active = Campaign {
            id: 2,
            artist_id: 1,
            name: None,
            active: None,
            vinyl_sold_count: None,
            campaign_start_date: None,
            campaign_end_date: None,
            progress: None,
            album_id: None,
            deleted_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(!active.is_deleted());
    }

    #[test]
    fn validate_vinyl_sold_count() {
        assert!(Campaign::validate_vinyl_sold_count(0));
        assert!(Campaign::validate_vinyl_sold_count(50));
        assert!(Campaign::validate_vinyl_sold_count(crate::constants::VINYL_TARGET));
        assert!(!Campaign::validate_vinyl_sold_count(crate::constants::VINYL_TARGET + 1));
        assert!(!Campaign::validate_vinyl_sold_count(-1));
    }

    #[test]
    fn create_campaign_request_serialization() {
        let req = CreateCampaignRequest {
            artist_id: 1,
            name: "Test Campaign".to_string(),
            vinyl_sold_count: Some(50),
            track_list: vec![SongEntry {
                name: "Track 1".to_string(),
                duration: Some("3:30".to_string()),
            }],
            artist_links: vec![],
            layout_style: Some(0),
            description: Some("A test".to_string()),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Test Campaign"));
        assert!(json.contains("Track 1"));
    }
}
