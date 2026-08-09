use serde::{Deserialize, Serialize};

use crate::auth::roles::Role;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionResource {
    pub resource: String,
    pub can_create: bool,
    pub can_read: bool,
    pub can_update: bool,
    pub can_delete: bool,
}

pub fn is_admin(role: &Role) -> bool {
    matches!(role, Role::Admin | Role::SuperAdmin)
}

pub fn is_artist_or_above(role: &Role) -> bool {
    matches!(
        role,
        Role::Admin | Role::SuperAdmin | Role::Artist
    )
}

pub fn is_customer_or_above(role: &Role) -> bool {
    matches!(
        role,
        Role::Admin | Role::SuperAdmin | Role::Artist | Role::Customer | Role::User
    )
}

pub fn has_permission(
    resource: &str,
    action: &str,
    permissions: &[PermissionResource],
) -> bool {
    permissions
        .iter()
        .any(|p| {
            p.resource == resource
                && match action {
                    "create" => p.can_create,
                    "read" => p.can_read,
                    "update" => p.can_update,
                    "delete" => p.can_delete,
                    _ => false,
                }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_admin_returns_true_for_admin() {
        assert!(is_admin(&Role::Admin));
        assert!(is_admin(&Role::SuperAdmin));
        assert!(!is_admin(&Role::Artist));
        assert!(!is_admin(&Role::User));
    }

    #[test]
    fn test_is_artist_or_above() {
        assert!(is_artist_or_above(&Role::Admin));
        assert!(is_artist_or_above(&Role::SuperAdmin));
        assert!(is_artist_or_above(&Role::Artist));
        assert!(!is_artist_or_above(&Role::Customer));
        assert!(!is_artist_or_above(&Role::User));
    }

    #[test]
    fn test_is_customer_or_above() {
        assert!(is_customer_or_above(&Role::Admin));
        assert!(is_customer_or_above(&Role::SuperAdmin));
        assert!(is_customer_or_above(&Role::Artist));
        assert!(is_customer_or_above(&Role::Customer));
        assert!(is_customer_or_above(&Role::User));
        assert!(!is_customer_or_above(&Role::Staff));
    }

    #[test]
    fn has_permission_creates() {
        let perms = vec![PermissionResource {
            resource: "Campaign".to_string(),
            can_create: true,
            can_read: true,
            can_update: false,
            can_delete: false,
        }];
        assert!(has_permission("Campaign", "create", &perms));
        assert!(!has_permission("Campaign", "update", &perms));
    }

    #[test]
    fn has_permission_wrong_resource() {
        let perms = vec![PermissionResource {
            resource: "News".to_string(),
            can_create: true,
            can_read: true,
            can_update: true,
            can_delete: true,
        }];
        assert!(!has_permission("Campaign", "create", &perms));
    }

    #[test]
    fn has_permission_empty_list() {
        let perms: Vec<PermissionResource> = vec![];
        assert!(!has_permission("Campaign", "create", &perms));
    }

    #[test]
    fn has_permission_invalid_action() {
        let perms = vec![PermissionResource {
            resource: "Campaign".to_string(),
            can_create: true,
            can_read: true,
            can_update: true,
            can_delete: true,
        }];
        assert!(!has_permission("Campaign", "execute", &perms));
    }

    #[test]
    fn permission_resource_serialization() {
        let perm = PermissionResource {
            resource: "Campaign".to_string(),
            can_create: true,
            can_read: true,
            can_update: false,
            can_delete: false,
        };
        let json = serde_json::to_string(&perm).expect("should serialize");
        assert!(json.contains("\"Campaign\""));
        assert!(json.contains("\"can_create\":true"));
    }
}
