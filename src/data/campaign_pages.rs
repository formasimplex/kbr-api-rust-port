use sqlx::FromRow;

use crate::models::campaign_page::CampaignPage;

#[derive(Debug, FromRow)]
pub struct CampaignPageRow {
    pub id: i64,
    pub campaign_id: i64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub page_type: Option<i32>,
    pub inventory_item_id: Option<String>,
    pub inventory_url: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<CampaignPageRow> for CampaignPage {
    fn from(row: CampaignPageRow) -> Self {
        CampaignPage {
            id: row.id,
            campaign_id: row.campaign_id,
            title: row.title,
            description: row.description,
            page_type: row.page_type,
            inventory_item_id: row.inventory_item_id,
            inventory_url: row.inventory_url,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const CAMPAIGN_PAGE_SELECT: &str =
    r#"SELECT id, campaign_id, title, description, page_type,
       inventory_item_id, inventory_url, created_at, updated_at FROM campaign_pages"#;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<CampaignPage>, sqlx::Error> {
    let rows = sqlx::query_as::<_, CampaignPageRow>(
        &format!("{} ORDER BY id", CAMPAIGN_PAGE_SELECT),
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<CampaignPage>, sqlx::Error> {
    let row = sqlx::query_as::<_, CampaignPageRow>(
        &format!("{} WHERE id = $1", CAMPAIGN_PAGE_SELECT),
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn by_campaign_id(
    pool: &sqlx::PgPool,
    campaign_id: i64,
) -> Result<Option<CampaignPage>, sqlx::Error> {
    let row = sqlx::query_as::<_, CampaignPageRow>(
        &format!("{} WHERE campaign_id = $1", CAMPAIGN_PAGE_SELECT),
    )
    .bind(campaign_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn find_by_inventory_item_id(
    pool: &sqlx::PgPool,
    inventory_item_id: &str,
) -> Result<Option<CampaignPage>, sqlx::Error> {
    let row = sqlx::query_as::<_, CampaignPageRow>(
        &format!("{} WHERE inventory_item_id = $1", CAMPAIGN_PAGE_SELECT),
    )
    .bind(inventory_item_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
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
