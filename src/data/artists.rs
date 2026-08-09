use crate::models::artist::Artist;

const ARTIST_COLUMNS: &str = r#"id, name, genre, bio, user_id, prospect,
    "spotifyId" AS spotify_id, "subHeading" AS sub_heading, intro,
    created_at, updated_at"#;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Artist>, sqlx::Error> {
    sqlx::query_as::<_, Artist>(
        &format!(r"SELECT {} FROM artists ORDER BY id", ARTIST_COLUMNS),
    )
    .fetch_all(pool)
    .await
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<Artist>, sqlx::Error> {
    sqlx::query_as::<_, Artist>(
        &format!(r"SELECT {} FROM artists WHERE id = $1", ARTIST_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn create(
    pool: &sqlx::PgPool,
    name: &str,
    genre: &Option<String>,
    bio: &Option<String>,
    prospect: Option<bool>,
    spotify_id: &Option<String>,
    sub_heading: &Option<String>,
    intro: &Option<String>,
    now: chrono::NaiveDateTime,
) -> Result<Artist, sqlx::Error> {
    sqlx::query_as::<_, Artist>(
        &format!(
            r#"INSERT INTO artists (name, genre, bio, prospect, "spotifyId", "subHeading", intro, created_at, updated_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               RETURNING {}"#,
            ARTIST_COLUMNS
        ),
    )
    .bind(name)
    .bind(genre)
    .bind(bio)
    .bind(prospect)
    .bind(spotify_id)
    .bind(sub_heading)
    .bind(intro)
    .bind(now)
    .bind(now)
    .fetch_one(pool)
    .await
}

pub async fn create_with_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    name: &str,
    user_id: i64,
    now: chrono::NaiveDateTime,
) -> Result<Artist, sqlx::Error> {
    sqlx::query_as::<_, Artist>(
        &format!(
            r#"INSERT INTO artists (name, user_id, prospect, created_at, updated_at)
               VALUES ($1, $2, true, $3, $3)
               RETURNING {}"#,
            ARTIST_COLUMNS
        ),
    )
    .bind(name)
    .bind(user_id)
    .bind(now)
    .fetch_one(&mut **tx)
    .await
}

pub async fn update(
    pool: &sqlx::PgPool,
    id: i64,
    name: &Option<String>,
    genre: &Option<String>,
    bio: &Option<String>,
    spotify_id: &Option<String>,
    sub_heading: &Option<String>,
    intro: &Option<String>,
    now: chrono::NaiveDateTime,
) -> Result<Option<Artist>, sqlx::Error> {
    sqlx::query_as::<_, Artist>(
        &format!(
            r#"UPDATE artists SET
               name = COALESCE($1, name),
               genre = COALESCE($2, genre),
               bio = COALESCE($3, bio),
               "spotifyId" = COALESCE($4, "spotifyId"),
               "subHeading" = COALESCE($5, "subHeading"),
               intro = COALESCE($6, intro),
               updated_at = $7
           WHERE id = $8
           RETURNING {}"#,
            ARTIST_COLUMNS
        ),
    )
    .bind(name)
    .bind(genre)
    .bind(bio)
    .bind(spotify_id)
    .bind(sub_heading)
    .bind(intro)
    .bind(now)
    .bind(id)
    .fetch_optional(pool)
    .await
}
