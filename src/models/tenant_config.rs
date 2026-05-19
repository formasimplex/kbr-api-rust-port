use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct TenantConfig {
    pub tenant_id: Uuid,
    pub logo_url: Option<String>,
    pub short_name: String,
    pub long_name: String,
    pub footer_logo_url: Option<String>,
    pub contact_email: String,
    pub site_header_description: String,
    pub deleted_at: Option<DateTime<Utc>>,
    pub insta_url: Option<String>,
    pub twitter_url: Option<String>,
    pub tiktok_url: Option<String>,
    pub spotify_id: Option<String>,
    pub featured_artist_id: Option<i64>,
    pub mantine_theme: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTenantConfigRequest {
    pub logo_url: Option<String>,
    pub short_name: String,
    pub long_name: String,
    pub footer_logo_url: Option<String>,
    pub contact_email: String,
    pub site_header_description: String,
    pub insta_url: Option<String>,
    pub twitter_url: Option<String>,
    pub tiktok_url: Option<String>,
    pub spotify_id: Option<String>,
    pub featured_artist_id: Option<i64>,
    pub mantine_theme: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTenantConfigRequest {
    pub logo_url: Option<String>,
    pub short_name: Option<String>,
    pub long_name: Option<String>,
    pub footer_logo_url: Option<String>,
    pub contact_email: Option<String>,
    pub site_header_description: Option<String>,
    pub insta_url: Option<String>,
    pub twitter_url: Option<String>,
    pub tiktok_url: Option<String>,
    pub spotify_id: Option<String>,
    pub featured_artist_id: Option<i64>,
    pub mantine_theme: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TenantConfigResponse {
    pub tenant_id: Uuid,
    pub logo_url: Option<String>,
    pub short_name: String,
    pub long_name: String,
    pub footer_logo_url: Option<String>,
    pub contact_email: String,
    pub site_header_description: String,
    pub insta_url: Option<String>,
    pub twitter_url: Option<String>,
    pub tiktok_url: Option<String>,
    pub spotify_id: Option<String>,
    pub mantine_theme: Option<serde_json::Value>,
}

impl TenantConfig {
    pub fn to_response(&self) -> TenantConfigResponse {
        TenantConfigResponse {
            tenant_id: self.tenant_id,
            logo_url: self.logo_url.clone(),
            short_name: self.short_name.clone(),
            long_name: self.long_name.clone(),
            footer_logo_url: self.footer_logo_url.clone(),
            contact_email: self.contact_email.clone(),
            site_header_description: self.site_header_description.clone(),
            insta_url: self.insta_url.clone(),
            twitter_url: self.twitter_url.clone(),
            tiktok_url: self.tiktok_url.clone(),
            spotify_id: self.spotify_id.clone(),
            mantine_theme: self.mantine_theme.clone(),
        }
    }

    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_config_is_deleted() {
        let config = TenantConfig {
            tenant_id: Uuid::new_v4(),
            logo_url: None,
            short_name: "X".to_string(),
            long_name: "X".to_string(),
            footer_logo_url: None,
            contact_email: "x@x.com".to_string(),
            site_header_description: "X".to_string(),
            deleted_at: Some(Utc::now()),
            insta_url: None,
            twitter_url: None,
            tiktok_url: None,
            spotify_id: None,
            featured_artist_id: None,
            mantine_theme: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert!(config.is_deleted());
    }
}
