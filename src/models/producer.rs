use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Producer {
    pub id: i64,
    pub description: Option<String>,
    pub producer_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProducerRequest {
    pub producer_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProducerRequest {
    pub producer_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProducerResponse {
    pub id: i64,
    pub producer_name: String,
    pub description: Option<String>,
}

impl Producer {
    pub fn to_response(&self) -> ProducerResponse {
        ProducerResponse {
            id: self.id,
            producer_name: self.producer_name.clone(),
            description: self.description.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn producer_to_response() {
        let producer = Producer {
            id: 1,
            description: Some("Vinyl presser".to_string()),
            producer_name: "Test Pressing".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = producer.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.producer_name, "Test Pressing");
    }
}
