use crate::auth::jwt::encode_token_with_role;

pub const TEST_SECRET: &str = "test-secret-key";

/// Generate a test JWT token for an admin user (user_id=1).
pub fn admin_token() -> String {
    encode_token_with_role(1, TEST_SECRET, 3, Some("admin".to_string()), 1).unwrap()
}

/// Generate a test JWT token for a regular user.
pub fn user_token(user_id: i64) -> String {
    encode_token_with_role(user_id, TEST_SECRET, 3, Some("user".to_string()), 1).unwrap()
}

/// Generate a test JWT token for an artist user.
pub fn artist_token(user_id: i64) -> String {
    encode_token_with_role(user_id, TEST_SECRET, 3, Some("artist".to_string()), 1).unwrap()
}

/// Generate a test JWT token for a customer user.
pub fn customer_token(user_id: i64) -> String {
    encode_token_with_role(user_id, TEST_SECRET, 3, Some("customer".to_string()), 1).unwrap()
}

/// Generate a test JWT token with a custom role.
pub fn token_with_role(user_id: i64, role: &str) -> String {
    encode_token_with_role(user_id, TEST_SECRET, 3, Some(role.to_string()), 1).unwrap()
}
