use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::error::AppError;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Claims {
    pub user_id: i64,
    pub exp: u64,
}

pub fn encode_token(user_id: i64, secret: &str, expiry_days: u64) -> Result<String, AppError> {
    let exp = Utc::now()
        .checked_add_signed(chrono::TimeDelta::days(expiry_days as i64))
        .expect("valid timestamp")
        .timestamp() as u64;

    let claims = Claims { user_id, exp };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Jwt(e.to_string()))
}

pub fn decode_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    let validation = Validation::default();

    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|e| AppError::Jwt(e.to_string()))
}

pub fn create_expired_token(user_id: i64, secret: &str) -> String {
    let claims = Claims {
        user_id,
        exp: Utc::now().timestamp() as u64 - 600, // expired 10 minutes ago (beyond leeway)
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .expect("should encode")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-secret-key-for-jwt-signing";

    #[test]
    fn encode_token_returns_valid_jwt() {
        let token = encode_token(42, SECRET, 3).expect("should encode");
        // JWT has 3 parts separated by dots
        assert_eq!(token.split('.').count(), 3);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let token = encode_token(42, SECRET, 3).expect("should encode");
        let claims = decode_token(&token, SECRET).expect("should decode");
        assert_eq!(claims.user_id, 42);
    }

    #[test]
    fn decode_preserves_user_id() {
        let token = encode_token(12345, SECRET, 7).expect("should encode");
        let claims = decode_token(&token, SECRET).expect("should decode");
        assert_eq!(claims.user_id, 12345);
    }

    #[test]
    fn decode_expired_token_fails() {
        let token = create_expired_token(42, SECRET);
        let result = decode_token(&token, SECRET);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, AppError::Jwt(_)));
    }

    #[test]
    fn decode_with_wrong_secret_fails() {
        let token = encode_token(42, SECRET, 3).expect("should encode");
        let result = decode_token(&token, "wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn decode_tampered_token_fails() {
        let token = encode_token(42, SECRET, 3).expect("should encode");
        let parts: Vec<&str> = token.split('.').collect();
        // Tamper with the payload by flipping a bit
        let tampered: String = token.chars().enumerate().map(|(i, c)| {
            if i > 0 && i < parts[0].len() + parts[1].len() + 1 {
                // In the payload section, flip a bit
                (c as u8 ^ 0x01) as char
            } else {
                c
            }
        }).collect();

        let result = decode_token(&tampered, SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn decode_empty_string_fails() {
        let result = decode_token("", SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn decode_random_string_fails() {
        let result = decode_token("not.a.jwt.token", SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn claims_are_serializable() {
        let claims = Claims {
            user_id: 42,
            exp: 9999999999,
        };
        let json = serde_json::to_string(&claims).expect("should serialize");
        assert!(json.contains("\"user_id\":42"));
        assert!(json.contains("\"exp\":9999999999"));
    }

    #[test]
    fn different_expiry_days_works() {
        let token_1day = encode_token(1, SECRET, 1).expect("should encode");
        let claims_1day = decode_token(&token_1day, SECRET).expect("should decode");

        let token_30days = encode_token(1, SECRET, 30).expect("should encode");
        let claims_30days = decode_token(&token_30days, SECRET).expect("should decode");

        // 30-day token should expire later than 1-day token
        assert!(claims_30days.exp > claims_1day.exp);
    }
}
