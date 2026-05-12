use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignPage {
    pub id: i64,
    pub campaign_id: i64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub page_type: Option<i32>,
    pub inventory_item_id: Option<String>,
    pub inventory_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCampaignPageRequest {
    pub campaign_id: i64,
    pub title: String,
    pub description: String,
    pub page_type: Option<i32>,
    pub inventory_item_id: Option<String>,
    pub inventory_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCampaignPageRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub page_type: Option<i32>,
    pub inventory_item_id: Option<String>,
    pub inventory_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CampaignPageResponse {
    pub id: i64,
    pub campaign_id: i64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub page_type: Option<i32>,
    pub inventory_url: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CampaignPage {
    pub fn to_response(&self) -> CampaignPageResponse {
        CampaignPageResponse {
            id: self.id,
            campaign_id: self.campaign_id,
            title: self.title.clone(),
            description: self.description.clone(),
            page_type: self.page_type,
            inventory_url: self.inventory_url.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn campaign_page_to_response() {
        let page = CampaignPage {
            id: 1,
            campaign_id: 5,
            title: Some("My Page".to_string()),
            description: Some("Description here".to_string()),
            page_type: Some(0),
            inventory_item_id: Some("item-123".to_string()),
            inventory_url: Some("https://example.com/item".to_string()),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = page.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.title, Some("My Page".to_string()));
        assert_eq!(resp.page_type, Some(0));
    }
}
