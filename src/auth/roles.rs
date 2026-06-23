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
    fn role_serialization_snake_case() {
        let role = Role::SuperAdmin;
        let json = serde_json::to_string(&role).expect("should serialize");
        assert_eq!(json, "\"super_admin\"");
    }
}
