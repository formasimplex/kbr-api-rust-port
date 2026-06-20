use sqlx::FromRow;

use crate::models::permission::Permission;

#[derive(Debug, FromRow)]
pub struct PermissionRow {
    pub id: i64,
    pub resource: Option<String>,
    pub can_create: bool,
    pub can_read: bool,
    pub can_update: bool,
    pub can_delete: bool,
    pub user_id: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<PermissionRow> for Permission {
    fn from(row: PermissionRow) -> Self {
        Permission {
            id: row.id,
            resource: row.resource,
            can_create: row.can_create,
            can_read: row.can_read,
            can_update: row.can_update,
            can_delete: row.can_delete,
            user_id: row.user_id,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Permission>, sqlx::Error> {
    let rows = sqlx::query_as::<_, PermissionRow>(
        r"SELECT id, resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at FROM permissions"
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<Permission>, sqlx::Error> {
    let row = sqlx::query_as::<_, PermissionRow>(
        r"SELECT id, resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at FROM permissions WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}

pub async fn create(
    pool: &sqlx::PgPool,
    resource: &str,
    can_create: bool,
    can_read: bool,
    can_update: bool,
    can_delete: bool,
    user_id: i64,
    now: chrono::NaiveDateTime,
) -> Result<Permission, sqlx::Error> {
    let row = sqlx::query_as::<_, PermissionRow>(
        r"INSERT INTO permissions (resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at"
    )
    .bind(resource)
    .bind(can_create)
    .bind(can_read)
    .bind(can_update)
    .bind(can_delete)
    .bind(user_id)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await?;

    Ok(row.into())
}

pub async fn update(
    pool: &sqlx::PgPool,
    id: i64,
    can_create: Option<bool>,
    can_read: Option<bool>,
    can_update: Option<bool>,
    can_delete: Option<bool>,
    now: chrono::NaiveDateTime,
) -> Result<Option<Permission>, sqlx::Error> {
    let row = sqlx::query_as::<_, PermissionRow>(
        r"UPDATE permissions
           SET can_create = COALESCE($1, can_create),
               can_read = COALESCE($2, can_read),
               can_update = COALESCE($3, can_update),
               can_delete = COALESCE($4, can_delete),
               updated_at = $5
           WHERE id = $6
           RETURNING id, resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at"
    )
    .bind(can_create)
    .bind(can_read)
    .bind(can_update)
    .bind(can_delete)
    .bind(now)
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}
