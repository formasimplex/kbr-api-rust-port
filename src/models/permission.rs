use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Permission {
    pub id: i64,
    pub resource: Option<String>,
    pub can_create: bool,
    pub can_read: bool,
    pub can_update: bool,
    pub can_delete: bool,
    pub user_id: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreatePermissionRequest {
    pub resource: String,
    pub can_create: Option<bool>,
    pub can_read: Option<bool>,
    pub can_update: Option<bool>,
    pub can_delete: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdatePermissionRequest {
    pub can_create: Option<bool>,
    pub can_read: Option<bool>,
    pub can_update: Option<bool>,
    pub can_delete: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PermissionResponse {
    pub id: i64,
    pub resource: Option<String>,
    pub can_create: bool,
    pub can_read: bool,
    pub can_update: bool,
    pub can_delete: bool,
    pub user_id: i64,
}

impl Permission {
    pub fn to_response(&self) -> PermissionResponse {
        PermissionResponse {
            id: self.id,
            resource: self.resource.clone(),
            can_create: self.can_create,
            can_read: self.can_read,
            can_update: self.can_update,
            can_delete: self.can_delete,
            user_id: self.user_id,
        }
    }

    pub fn create_artist_permissions(_user_id: i64) -> Vec<CreatePermissionRequest> {
        vec![
            CreatePermissionRequest {
                resource: "Artist".to_string(),
                can_create: Some(false),
                can_read: Some(true),
                can_update: Some(true),
                can_delete: Some(false),
            },
            CreatePermissionRequest {
                resource: "Campaign".to_string(),
                can_create: Some(true),
                can_read: Some(true),
                can_update: Some(true),
                can_delete: Some(true),
            },
            CreatePermissionRequest {
                resource: "Song".to_string(),
                can_create: Some(true),
                can_read: Some(true),
                can_update: Some(true),
                can_delete: Some(true),
            },
            CreatePermissionRequest {
                resource: "Album".to_string(),
                can_create: Some(true),
                can_read: Some(true),
                can_update: Some(true),
                can_delete: Some(true),
            },
            CreatePermissionRequest {
                resource: "KBREvent".to_string(),
                can_create: Some(true),
                can_read: Some(true),
                can_update: Some(true),
                can_delete: Some(true),
            },
            CreatePermissionRequest {
                resource: "News".to_string(),
                can_create: Some(true),
                can_read: Some(true),
                can_update: Some(true),
                can_delete: Some(true),
            },
        ]
    }

    pub fn create_news_permissions(_user_id: i64) -> Vec<CreatePermissionRequest> {
        vec![CreatePermissionRequest {
            resource: "News".to_string(),
            can_create: Some(true),
            can_read: Some(true),
            can_update: Some(true),
            can_delete: Some(true),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_to_response() {
        let perm = Permission {
            id: 1,
            resource: Some("Campaign".to_string()),
            can_create: true,
            can_read: true,
            can_update: false,
            can_delete: false,
            user_id: 42,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = perm.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.resource, Some("Campaign".to_string()));
        assert!(resp.can_create);
        assert!(resp.can_read);
        assert!(!resp.can_update);
        assert!(!resp.can_delete);
        assert_eq!(resp.user_id, 42);
    }

    #[test]
    fn permission_to_response_null_resource() {
        let perm = Permission {
            id: 2,
            resource: None,
            can_create: false,
            can_read: true,
            can_update: false,
            can_delete: false,
            user_id: 1,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = perm.to_response();
        assert_eq!(resp.resource, None);
    }

    #[test]
    fn create_artist_permissions_returns_six() {
        let perms = Permission::create_artist_permissions(1);
        assert_eq!(perms.len(), 6);
        let resources: Vec<_> = perms.iter().map(|p| p.resource.as_str()).collect();
        assert!(resources.contains(&"Artist"));
        assert!(resources.contains(&"Campaign"));
        assert!(resources.contains(&"Song"));
        assert!(resources.contains(&"Album"));
        assert!(resources.contains(&"KBREvent"));
        assert!(resources.contains(&"News"));
    }

    #[test]
    fn create_artist_permissions_artist_resource() {
        let perms = Permission::create_artist_permissions(1);
        let artist_perm = perms.iter().find(|p| p.resource == "Artist").unwrap();
        assert!(!artist_perm.can_create.unwrap_or(false));
        assert!(artist_perm.can_read.unwrap_or(false));
        assert!(artist_perm.can_update.unwrap_or(false));
        assert!(!artist_perm.can_delete.unwrap_or(false));
    }

    #[test]
    fn create_artist_permissions_campaign_resource_full_crud() {
        let perms = Permission::create_artist_permissions(1);
        let campaign_perm = perms.iter().find(|p| p.resource == "Campaign").unwrap();
        assert!(campaign_perm.can_create.unwrap_or(false));
        assert!(campaign_perm.can_read.unwrap_or(false));
        assert!(campaign_perm.can_update.unwrap_or(false));
        assert!(campaign_perm.can_delete.unwrap_or(false));
    }

    #[test]
    fn create_news_permissions_returns_one() {
        let perms = Permission::create_news_permissions(1);
        assert_eq!(perms.len(), 1);
        assert_eq!(perms[0].resource, "News");
        assert!(perms[0].can_create.unwrap_or(false));
        assert!(perms[0].can_read.unwrap_or(false));
        assert!(perms[0].can_update.unwrap_or(false));
        assert!(perms[0].can_delete.unwrap_or(false));
    }

    #[test]
    fn create_permission_request_serialization() {
        let req = CreatePermissionRequest {
            resource: "Album".to_string(),
            can_create: Some(true),
            can_read: Some(true),
            can_update: Some(false),
            can_delete: Some(false),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Album"));
        assert!(json.contains("true"));
    }

    #[test]
    fn update_permission_request_serialization() {
        let req = UpdatePermissionRequest {
            can_create: Some(true),
            can_read: None,
            can_update: Some(true),
            can_delete: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("can_create"));
    }
}
