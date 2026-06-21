use sqlx::FromRow;

use crate::models::artist::Artist;

#[derive(Debug, FromRow)]
pub struct ArtistRow {
    pub id: i64,
    pub name: Option<String>,
    pub genre: Option<String>,
    pub bio: Option<String>,
    pub user_id: Option<i64>,
    pub prospect: Option<bool>,
    pub spotify_id: Option<String>,
    pub sub_heading: Option<String>,
    pub intro: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<ArtistRow> for Artist {
    fn from(row: ArtistRow) -> Self {
        Artist {
            id: row.id,
            name: row.name,
            genre: row.genre,
            bio: row.bio,
            user_id: row.user_id,
            prospect: row.prospect,
            spotify_id: row.spotify_id,
            sub_heading: row.sub_heading,
            intro: row.intro,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const ARTIST_COLUMNS: &str = r#"id, name, genre, bio, user_id, prospect,
    "spotifyId" AS spotify_id, "subHeading" AS sub_heading, intro,
    created_at, updated_at"#;

pub async fn list(pool: &sqlx::PgPool) -> Result<Vec<Artist>, sqlx::Error> {
    let rows = sqlx::query_as::<_, ArtistRow>(
        &format!(r"SELECT {} FROM artists ORDER BY id", ARTIST_COLUMNS),
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn by_id(pool: &sqlx::PgPool, id: i64) -> Result<Option<Artist>, sqlx::Error> {
    let row = sqlx::query_as::<_, ArtistRow>(
        &format!(r"SELECT {} FROM artists WHERE id = $1", ARTIST_COLUMNS),
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| r.into()))
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
    let row = sqlx::query_as::<_, ArtistRow>(
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
    .await?;

    Ok(row.into())
}

pub async fn create_with_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    name: &str,
    user_id: i64,
    now: chrono::NaiveDateTime,
) -> Result<ArtistRow, sqlx::Error> {
    let row = sqlx::query_as::<_, ArtistRow>(
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
    .await?;

    Ok(row)
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
    let row = sqlx::query_as::<_, ArtistRow>(
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
    .await?;

    Ok(row.map(|r| r.into()))
}
