use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    SuperAdmin,
    Artist,
    Customer,
    User,
    Staff,
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "admin" => Ok(Self::Admin),
            "super_admin" => Ok(Self::SuperAdmin),
            "artist" => Ok(Self::Artist),
            "customer" => Ok(Self::Customer),
            "user" => Ok(Self::User),
            "staff" => Ok(Self::Staff),
            _ => Err(format!("Unknown role: {}", s)),
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admin => write!(f, "admin"),
            Self::SuperAdmin => write!(f, "super_admin"),
            Self::Artist => write!(f, "artist"),
            Self::Customer => write!(f, "customer"),
            Self::User => write!(f, "user"),
            Self::Staff => write!(f, "staff"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionResource {
    pub resource: String,
    pub can_create: bool,
    pub can_read: bool,
    pub can_update: bool,
    pub can_delete: bool,
}

pub const RESOURCES: ResourceNames = ResourceNames {
    album: "Album",
    song: "Song",
    artist: "Artist",
    campaign: "Campaign",
    kbr_event: "KBREvent",
    news: "News",
    user: "User",
};

pub struct ResourceNames {
    pub album: &'static str,
    pub song: &'static str,
    pub artist: &'static str,
    pub campaign: &'static str,
    pub kbr_event: &'static str,
    pub news: &'static str,
    pub user: &'static str,
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
    fn role_from_str_valid() {
        assert_eq!("admin".parse::<Role>(), Ok(Role::Admin));
        assert_eq!("super_admin".parse::<Role>(), Ok(Role::SuperAdmin));
        assert_eq!("artist".parse::<Role>(), Ok(Role::Artist));
        assert_eq!("customer".parse::<Role>(), Ok(Role::Customer));
        assert_eq!("user".parse::<Role>(), Ok(Role::User));
        assert_eq!("staff".parse::<Role>(), Ok(Role::Staff));
    }

    #[test]
    fn role_from_str_invalid() {
        assert!("moderator".parse::<Role>().is_err());
        assert!("".parse::<Role>().is_err());
    }

    #[test]
    fn role_display() {
        assert_eq!(Role::Admin.to_string(), "admin");
        assert_eq!(Role::SuperAdmin.to_string(), "super_admin");
        assert_eq!(Role::Artist.to_string(), "artist");
    }

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
    fn resources_constant_has_all_values() {
        assert_eq!(RESOURCES.album, "Album");
        assert_eq!(RESOURCES.song, "Song");
        assert_eq!(RESOURCES.artist, "Artist");
        assert_eq!(RESOURCES.campaign, "Campaign");
        assert_eq!(RESOURCES.kbr_event, "KBREvent");
        assert_eq!(RESOURCES.news, "News");
        assert_eq!(RESOURCES.user, "User");
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

    #[test]
    fn role_serialization_snake_case() {
        let role = Role::SuperAdmin;
        let json = serde_json::to_string(&role).expect("should serialize");
        assert_eq!(json, "\"super_admin\"");
    }
}
