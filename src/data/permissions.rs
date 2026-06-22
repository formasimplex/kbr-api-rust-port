use crate::models::permission::Permission;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Permission>, sqlx::Error> {
    sqlx::query_as::<_, Permission>(
        r"SELECT id, resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at FROM permissions"
    )
    .fetch_all(pool)
    .await
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<Permission>, sqlx::Error> {
    sqlx::query_as::<_, Permission>(
        r"SELECT id, resource, can_create, can_read, can_update, can_delete, user_id, created_at, updated_at FROM permissions WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(pool)
    .await
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
    sqlx::query_as::<_, Permission>(
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
    .await
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
    sqlx::query_as::<_, Permission>(
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
    .await
}
