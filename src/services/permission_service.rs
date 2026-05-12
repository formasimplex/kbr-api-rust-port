use crate::auth::roles::{has_permission, PermissionResource, RESOURCES};
use crate::error::AppError;
use crate::models::permission::{CreatePermissionRequest, Permission};

pub struct PermissionService;

impl PermissionService {
    pub fn validate_resource(resource: &str) -> Result<(), AppError> {
        let valid = [
            RESOURCES.album,
            RESOURCES.song,
            RESOURCES.artist,
            RESOURCES.campaign,
            RESOURCES.kbr_event,
            RESOURCES.news,
            RESOURCES.user,
        ];
        if !valid.contains(&resource) {
            return Err(AppError::Validation(format!(
                "Invalid resource: {}",
                resource
            )));
        }
        Ok(())
    }

    pub fn check_permission(
        permissions: &[PermissionResource],
        resource: &str,
        action: &str,
    ) -> Result<(), AppError> {
        if has_permission(resource, action, permissions) {
            Ok(())
        } else {
            Err(AppError::Forbidden(format!(
                "No permission to {} {}",
                action, resource
            )))
        }
    }

    pub fn get_all_resource_names() -> Vec<String> {
        vec![
            RESOURCES.album.to_string(),
            RESOURCES.song.to_string(),
            RESOURCES.artist.to_string(),
            RESOURCES.campaign.to_string(),
            RESOURCES.kbr_event.to_string(),
            RESOURCES.news.to_string(),
            RESOURCES.user.to_string(),
        ]
    }

    pub fn build_permission_resource(
        perm: &Permission,
    ) -> PermissionResource {
        PermissionResource {
            resource: perm.resource.clone().unwrap_or_default(),
            can_create: perm.can_create,
            can_read: perm.can_read,
            can_update: perm.can_update,
            can_delete: perm.can_delete,
        }
    }

    pub fn validate_create_request(req: &CreatePermissionRequest) -> Result<(), AppError> {
        Self::validate_resource(&req.resource)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_resource_valid() {
        assert!(PermissionService::validate_resource("Album").is_ok());
        assert!(PermissionService::validate_resource("Song").is_ok());
        assert!(PermissionService::validate_resource("Artist").is_ok());
        assert!(PermissionService::validate_resource("Campaign").is_ok());
        assert!(PermissionService::validate_resource("KBREvent").is_ok());
        assert!(PermissionService::validate_resource("News").is_ok());
        assert!(PermissionService::validate_resource("User").is_ok());
    }

    #[test]
    fn validate_resource_invalid() {
        let err = PermissionService::validate_resource("Invalid").unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn check_permission_granted() {
        let perms = vec![PermissionResource {
            resource: "Campaign".to_string(),
            can_create: true,
            can_read: true,
            can_update: false,
            can_delete: false,
        }];
        assert!(
            PermissionService::check_permission(&perms, "Campaign", "create")
                .is_ok()
        );
        assert!(
            PermissionService::check_permission(&perms, "Campaign", "read").is_ok()
        );
    }

    #[test]
    fn check_permission_denied() {
        let perms = vec![PermissionResource {
            resource: "Campaign".to_string(),
            can_create: false,
            can_read: true,
            can_update: false,
            can_delete: false,
        }];
        let err =
            PermissionService::check_permission(&perms, "Campaign", "create").unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[test]
    fn check_permission_wrong_resource() {
        let perms = vec![PermissionResource {
            resource: "News".to_string(),
            can_create: true,
            can_read: true,
            can_update: true,
            can_delete: true,
        }];
        let err =
            PermissionService::check_permission(&perms, "Campaign", "create").unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[test]
    fn get_all_resource_names() {
        let names = PermissionService::get_all_resource_names();
        assert_eq!(names.len(), 7);
        assert!(names.contains(&"Album".to_string()));
        assert!(names.contains(&"Song".to_string()));
        assert!(names.contains(&"Artist".to_string()));
        assert!(names.contains(&"Campaign".to_string()));
        assert!(names.contains(&"KBREvent".to_string()));
        assert!(names.contains(&"News".to_string()));
        assert!(names.contains(&"User".to_string()));
    }

    #[test]
    fn build_permission_resource() {
        let perm = Permission {
            id: 1,
            resource: Some("Campaign".to_string()),
            can_create: true,
            can_read: true,
            can_update: false,
            can_delete: false,
            user_id: 5,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let pr = PermissionService::build_permission_resource(&perm);
        assert_eq!(pr.resource, "Campaign");
        assert!(pr.can_create);
        assert!(!pr.can_update);
    }

    #[test]
    fn validate_create_request_valid() {
        let req = CreatePermissionRequest {
            resource: "Album".to_string(),
            can_create: Some(true),
            can_read: Some(true),
            can_update: Some(false),
            can_delete: Some(false),
        };
        assert!(PermissionService::validate_create_request(&req).is_ok());
    }

    #[test]
    fn validate_create_request_invalid_resource() {
        let req = CreatePermissionRequest {
            resource: "Invalid".to_string(),
            can_create: None,
            can_read: None,
            can_update: None,
            can_delete: None,
        };
        let err = PermissionService::validate_create_request(&req).unwrap_err();
        assert!(matches!(err, AppError::Validation(_)));
    }
}
