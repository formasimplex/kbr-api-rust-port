use sqlx::FromRow;

#[derive(Debug, FromRow)]
pub struct SubscribedArtistRow {
    pub id: i64,
    pub name: Option<String>,
    pub intro: Option<String>,
}

pub async fn subscribed_artists(
    pool: &sqlx::PgPool,
    user_id: i64,
) -> Result<Vec<SubscribedArtistRow>, sqlx::Error> {
    let rows = sqlx::query_as::<_, SubscribedArtistRow>(
        r#"
        SELECT DISTINCT artists.id, artists.name, artists.intro
        FROM artists
        INNER JOIN mail_subscribers ON mail_subscribers.artist_id = artists.id
        WHERE mail_subscribers.user_id = $1
          AND mail_subscribers.unsubscribed_at IS NULL
        ORDER BY artists.id
        "#
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}
