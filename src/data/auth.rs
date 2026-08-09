use crate::models::user::{User, UserRowNoPassword, USER_COLUMNS_NO_PASSWORD};

pub async fn find_by_id_no_password(pool: &sqlx::PgPool, id: i64) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query_as::<_, UserRowNoPassword>(
        &format!("SELECT {} FROM users WHERE id = $1", USER_COLUMNS_NO_PASSWORD),
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
}
