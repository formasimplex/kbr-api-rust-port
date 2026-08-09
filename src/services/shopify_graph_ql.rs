//! Shopify GraphQL operations
//!
//! Wraps [`ShopifyClient`] with typed GraphQL queries and mutations for
//! product management. Each method corresponds to a Shopify Admin GraphQL
//! operation used during campaign activation and merchandise sync.
//!
//! # Operations
//!
//! | Method | Type | Purpose |
//! |--------|------|---------|
//! | [`get_product`] | query | Fetch product details by ID |
//! | [`create_campaign_product`] | mutation | Create product with image media |
//! | [`create_product_variant`] | mutation | Create variant with inventory |
//! | [`get_location_id`] | query | Get first store location ID |
//! | [`get_publications_id`] | query | Get first publication ID |
//! | [`publish_product`] | mutation | Publish product to online store |

use crate::error::AppError;
use crate::services::shopify_client::ShopifyClient;

/// Shopify GraphQL operation wrapper.
///
/// Provides strongly-typed methods for common Shopify Admin GraphQL operations.
/// Each method constructs the raw GraphQL string, binds variables, and executes
/// via the underlying [`ShopifyClient`].
pub struct ShopifyGraphQl {
    client: ShopifyClient,
}

impl ShopifyGraphQl {
    /// Create a new ShopifyGraphQl wrapper around a ShopifyClient.
    pub fn new(client: ShopifyClient) -> Self {
        Self { client }
    }

    /// Fetch a single product by its Shopify GID.
    ///
    /// Returns product details including title, description, price range,
    /// online store URLs, and total inventory count.
    ///
    /// # Arguments
    ///
    /// * `id` — Shopify product GID (e.g. `"gid://shopify/Product/12345"`)
    pub async fn get_product(&self, id: &str) -> Result<serde_json::Value, AppError> {
        let query = r#"
            query {
                product(id: "gid://shopify/Product/{{ID}}") {
                    id
                    title
                    descriptionHtml
                    description
                    onlineStoreUrl
                    onlineStorePreviewUrl
                    priceRangeV2 {
                        maxVariantPrice { amount currencyCode }
                        minVariantPrice { amount currencyCode }
                    }
                    totalInventory
                }
            }
        "#
        .replace("{{ID}}", id);

        self.client.query(&query, serde_json::json!({})).await
    }

    /// Create a new product with image media in the KBR collection.
    ///
    /// Creates a "Vinyl" product type under vendor "KBR", joins it to the
    /// collection specified by `SHOPIFY_KBR_ID` env var, and attaches an
    /// image from the provided URL.
    ///
    /// # Arguments
    ///
    /// * `title` — Product title (campaign page title)
    /// * `description` — HTML description
    /// * `image_url` — Publicly accessible image URL
    pub async fn create_campaign_product(
        &self,
        title: &str,
        description: &str,
        image_url: &str,
    ) -> Result<serde_json::Value, AppError> {
        let kbr_id = std::env::var("SHOPIFY_KBR_ID").unwrap_or_default();

        let query = r#"
            mutation CreateProductWithNewMedia($input: ProductInput!, $media: [CreateMediaInput!]) {
                productCreate(input: $input, media: $media) {
                    product {
                        id
                        onlineStoreUrl
                        onlineStorePreviewUrl
                        handle
                        hasOnlyDefaultVariant
                        options(first: 1) { id name values }
                    }
                    userErrors { field message }
                }
            }
        "#;

        let variables = serde_json::json!({
            "input": {
                "collectionsToJoin": [kbr_id],
                "title": title,
                "vendor": "KBR",
                "productType": "Vinyl",
                "descriptionHtml": description,
            },
            "media": [
                {
                    "mediaContentType": "IMAGE",
                    "originalSource": image_url,
                    "alt": title,
                }
            ],
        });

        self.client.query(query, variables).await
    }

    /// Create a product variant with inventory tracking.
    ///
    /// Creates a single variant with configured price and inventory quantity
    /// at the specified location.
    ///
    /// # Arguments
    ///
    /// * `product_id` — Shopify product GID
    /// * `option_id` — Product option GID (typically the default "Title" option)
    /// * `location_id` — Shopify location GID for inventory
    pub async fn create_product_variant(
        &self,
        product_id: &str,
        option_id: &str,
        location_id: &str,
    ) -> Result<serde_json::Value, AppError> {
        let query = r#"
            mutation productVariantsBulkCreate($productId: ID!, $variants: [ProductVariantsBulkInput!]!) {
                productVariantsBulkCreate(productId: $productId, variants: $variants) {
                    product {
                        id
                        title
                        onlineStoreUrl
                        onlineStorePreviewUrl
                    }
                    productVariants {
                        id
                        metafields(first: 1) {
                            edges { node { namespace key value } }
                        }
                    }
                    userErrors { field message }
                }
            }
        "#;

        let variables = serde_json::json!({
            "productId": product_id,
            "variants": [
                {
                    "optionValues": [
                        { "name": "Vinyl", "optionId": option_id }
                    ],
                    "inventoryItem": {
                        "cost": crate::constants::SHOPIFY_VARIANT_PRICE,
                        "tracked": true,
                    },
                    "inventoryQuantities": [
                        { "locationId": location_id, "availableQuantity": crate::constants::SHOPIFY_INVENTORY_QUANTITY }
                    ],
                    "price": crate::constants::SHOPIFY_VARIANT_PRICE,
                }
            ],
        });

        self.client.query(query, variables).await
    }

    /// Fetch the first publication ID for the store.
    ///
    /// Used to determine where to publish products (typically "Web" / online store).
    pub async fn get_publications_id(&self) -> Result<String, AppError> {
        let query = r#"
            query publications {
                publications(first: 3) {
                    edges { node { id name } }
                }
            }
        "#;

        let resp = self.client.query(query, serde_json::json!({})).await?;
        resp.pointer("/data/publications/edges/0/node/id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Shopify("No publications found".to_string()))
    }

    /// Fetch the first store location ID.
    ///
    /// Used for assigning inventory quantities to a physical location.
    pub async fn get_location_id(&self) -> Result<String, AppError> {
        let query = r#"
            query {
                location {
                    id
                    name
                }
            }
        "#;

        let resp = self.client.query(query, serde_json::json!({})).await?;
        resp.pointer("/data/location/id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Shopify("No location found".to_string()))
    }

    /// Publish a product to a publication channel.
    ///
    /// Makes the product visible on the online store (or other publication channel).
    ///
    /// # Arguments
    ///
    /// * `product_id` — Shopify product GID
    /// * `publication_id` — Publication GID (from `get_publications_id`)
    pub async fn publish_product(
        &self,
        product_id: &str,
        publication_id: &str,
    ) -> Result<serde_json::Value, AppError> {
        let query = r#"
            mutation publishablePublish($id: ID!, $input: [PublicationInput!]!) {
                publishablePublish(id: $id, input: $input) {
                    publishable {
                        availablePublicationsCount { count }
                        resourcePublicationsCount { count }
                    }
                    shop { publicationCount }
                    userErrors { field message }
                }
            }
        "#;

        let variables = serde_json::json!({
            "id": product_id,
            "input": { "publicationId": publication_id },
        });

        self.client.query(query, variables).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_client() -> ShopifyClient {
        ShopifyClient::new("test.myshopify.com", "test-token").unwrap()
    }

    #[test]
    fn shopify_graphql_construction() {
        let client = test_client();
        let _sg = ShopifyGraphQl::new(client);
    }

    #[test]
    fn get_product_query_contains_id() {
        let client = test_client();
        let _sg = ShopifyGraphQl::new(client);

        let query = r#"
            query {
                product(id: "gid://shopify/Product/{{ID}}") {
                    id
                    title
                    descriptionHtml
                    description
                    onlineStoreUrl
                    onlineStorePreviewUrl
                    priceRangeV2 {
                        maxVariantPrice { amount currencyCode }
                        minVariantPrice { amount currencyCode }
                    }
                    totalInventory
                }
            }
        "#
        .replace("{{ID}}", "12345");

        assert!(query.contains("gid://shopify/Product/12345"));
        assert!(!query.contains("{{ID}}"));
    }

    #[test]
    fn create_campaign_product_variables_structure() {
        let kbr_id = "test-collection-id";
        let title = "Test Campaign";
        let description = "Test description";
        let image_url = "https://example.com/image.jpg";

        let variables = serde_json::json!({
            "input": {
                "collectionsToJoin": [kbr_id],
                "title": title,
                "vendor": "KBR",
                "productType": "Vinyl",
                "descriptionHtml": description,
            },
            "media": [
                {
                    "mediaContentType": "IMAGE",
                    "originalSource": image_url,
                    "alt": title,
                }
            ],
        });

        assert_eq!(variables["input"]["title"], "Test Campaign");
        assert_eq!(variables["input"]["vendor"], "KBR");
        assert_eq!(variables["input"]["productType"], "Vinyl");
        assert_eq!(variables["media"][0]["mediaContentType"], "IMAGE");
        assert_eq!(variables["media"][0]["originalSource"], "https://example.com/image.jpg");
    }

    #[test]
    fn create_product_variant_variables_structure() {
        let product_id = "gid://shopify/Product/123";
        let option_id = "gid://shopify/ProductOption/456";
        let location_id = "gid://shopify/Location/789";

        let variables = serde_json::json!({
            "productId": product_id,
            "variants": [
                {
                    "optionValues": [
                        { "name": "Vinyl", "optionId": option_id }
                    ],
                    "inventoryItem": {
                        "cost": crate::constants::SHOPIFY_VARIANT_PRICE,
                        "tracked": true,
                    },
                    "inventoryQuantities": [
                        { "locationId": location_id, "availableQuantity": crate::constants::SHOPIFY_INVENTORY_QUANTITY }
                    ],
                    "price": crate::constants::SHOPIFY_VARIANT_PRICE,
                }
            ],
        });

        assert_eq!(variables["variants"][0]["price"], crate::constants::SHOPIFY_VARIANT_PRICE);
        assert_eq!(variables["variants"][0]["inventoryQuantities"][0]["availableQuantity"], crate::constants::SHOPIFY_INVENTORY_QUANTITY);
        assert!(variables["variants"][0]["inventoryItem"]["tracked"].is_boolean());
    }

    #[test]
    fn get_publications_id_response_parsing() {
        let resp = serde_json::json!({
            "data": {
                "publications": {
                    "edges": [
                        { "node": { "id": "pub_123", "name": "Web" } }
                    ]
                }
            }
        });

        let id = resp.pointer("/data/publications/edges/0/node/id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        assert_eq!(id, Some("pub_123".to_string()));
    }

    #[test]
    fn get_publications_id_empty_response() {
        let resp = serde_json::json!({
            "data": {
                "publications": {
                    "edges": []
                }
            }
        });

        let id = resp.pointer("/data/publications/edges/0/node/id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        assert!(id.is_none());
    }

    #[test]
    fn get_location_id_response_parsing() {
        let resp = serde_json::json!({
            "data": {
                "location": {
                    "id": "loc_456",
                    "name": "Main Store"
                }
            }
        });

        let id = resp.pointer("/data/location/id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        assert_eq!(id, Some("loc_456".to_string()));
    }

    #[test]
    fn publish_product_variables_structure() {
        let product_id = "gid://shopify/Product/999";
        let publication_id = "pub_123";

        let variables = serde_json::json!({
            "id": product_id,
            "input": { "publicationId": publication_id },
        });

        assert_eq!(variables["id"], "gid://shopify/Product/999");
        assert_eq!(variables["input"]["publicationId"], "pub_123");
    }
}
