use crate::models::campaign_page::CampaignPage;

const CAMPAIGN_PAGE_SELECT: &str =
    r#"SELECT id, campaign_id, title, description, page_type,
       inventory_item_id, inventory_url, created_at, updated_at FROM campaign_pages"#;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<CampaignPage>, sqlx::Error> {
    sqlx::query_as::<_, CampaignPage>(
        &format!("{} ORDER BY id", CAMPAIGN_PAGE_SELECT),
    )
    .fetch_all(pool)
    .await
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<CampaignPage>, sqlx::Error> {
    sqlx::query_as::<_, CampaignPage>(
        &format!("{} WHERE id = $1", CAMPAIGN_PAGE_SELECT),
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn by_campaign_id(
    pool: &sqlx::PgPool,
    campaign_id: i64,
) -> Result<Option<CampaignPage>, sqlx::Error> {
    sqlx::query_as::<_, CampaignPage>(
        &format!("{} WHERE campaign_id = $1", CAMPAIGN_PAGE_SELECT),
    )
    .bind(campaign_id)
    .fetch_optional(pool)
    .await
}

pub async fn find_by_inventory_item_id(
    pool: &sqlx::PgPool,
    inventory_item_id: &str,
) -> Result<Option<CampaignPage>, sqlx::Error> {
    sqlx::query_as::<_, CampaignPage>(
        &format!("{} WHERE inventory_item_id = $1", CAMPAIGN_PAGE_SELECT),
    )
    .bind(inventory_item_id)
    .fetch_optional(pool)
    .await
}

pub async fn update_inventory(
    pool: &sqlx::PgPool,
    id: i64,
    inventory_item_id: &Option<String>,
    inventory_url: &Option<String>,
    now: chrono::NaiveDateTime,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"UPDATE campaign_pages SET
            inventory_item_id = $1,
            inventory_url = $2,
            updated_at = $3
        WHERE id = $4"#,
    )
    .bind(inventory_item_id)
    .bind(inventory_url)
    .bind(now)
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}
