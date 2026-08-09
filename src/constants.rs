//! Application-wide constants.
//!
//! Centralizes magic numbers used across handlers, services, and models
//! to improve maintainability and avoid duplication.

/// Default campaign duration in days from activation start date.
pub const CAMPAIGN_DURATION_DAYS: i64 = 45;

/// Maximum vinyl units per campaign (also the Shopify inventory target).
pub const VINYL_TARGET: i32 = 100;

/// Default Shopify product variant price in USD.
pub const SHOPIFY_VARIANT_PRICE: f64 = 23.0;

/// Default Shopify inventory quantity per variant.
pub const SHOPIFY_INVENTORY_QUANTITY: i32 = 100;
