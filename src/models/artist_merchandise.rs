use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, PartialEq)]
pub struct ArtistMerchandise {
    pub id: i64,
    pub artist_id: i64,
    pub producer_id: i64,
    pub merchandise_id: Option<String>,
    pub description: Option<String>,
    pub created_on_producer: Option<bool>,
    pub merch_title: String,
    pub merch_product_title: Option<String>,
    pub set_price: Option<f64>,
    pub cost_price: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateArtistMerchandiseRequest {
    pub artist_id: i64,
    pub producer_id: i64,
    pub merch_title: String,
    pub merch_product_title: Option<String>,
    pub description: Option<String>,
    pub set_price: Option<f64>,
    pub cost_price: Option<f64>,
    pub merchandise_id: Option<String>,
    pub created_on_producer: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateArtistMerchandiseRequest {
    pub merch_title: Option<String>,
    pub merch_product_title: Option<String>,
    pub description: Option<String>,
    pub set_price: Option<f64>,
    pub cost_price: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArtistMerchandiseResponse {
    pub id: i64,
    pub merch_title: String,
    pub merch_product_title: Option<String>,
    pub description: Option<String>,
    pub set_price: Option<f64>,
    pub cost_price: Option<f64>,
    pub created_on_producer: Option<bool>,
    pub merchandise_id: Option<String>,
}

impl ArtistMerchandise {
    pub fn to_response(&self) -> ArtistMerchandiseResponse {
        ArtistMerchandiseResponse {
            id: self.id,
            merch_title: self.merch_title.clone(),
            merch_product_title: self.merch_product_title.clone(),
            description: self.description.clone(),
            set_price: self.set_price,
            cost_price: self.cost_price,
            created_on_producer: self.created_on_producer,
            merchandise_id: self.merchandise_id.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artist_merchandise_to_response() {
        let merch = ArtistMerchandise {
            id: 1,
            artist_id: 5,
            producer_id: 2,
            merchandise_id: Some("shopify-123".to_string()),
            description: Some("T-shirt".to_string()),
            created_on_producer: Some(true),
            merch_title: "Band Tee".to_string(),
            merch_product_title: Some("Band Tee - Black".to_string()),
            set_price: Some(25.99),
            cost_price: Some(12.50),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let resp = merch.to_response();
        assert_eq!(resp.id, 1);
        assert_eq!(resp.merch_title, "Band Tee");
        assert_eq!(resp.set_price, Some(25.99));
    }
}
