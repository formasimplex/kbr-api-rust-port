use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq, Eq)]
pub struct Producer {
    pub id: i64,
    pub description: Option<String>,
    pub producer_name: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
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


