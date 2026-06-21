use sqlx::FromRow;

use crate::models::kbr_event_attendee::KbrEventAttendee;

#[derive(Debug, FromRow)]
pub struct EventAttendeeRow {
    pub id: i64,
    pub kbr_event_id: Option<i32>,
    pub mail_subscriber_id: Option<i32>,
    pub scan_count: Option<i32>,
    pub headcount: Option<i32>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

impl From<EventAttendeeRow> for KbrEventAttendee {
    fn from(row: EventAttendeeRow) -> Self {
        KbrEventAttendee {
            id: row.id,
            kbr_event_id: row.kbr_event_id,
            mail_subscriber_id: row.mail_subscriber_id,
            scan_count: row.scan_count,
            headcount: row.headcount,
            created_at: row.created_at.and_utc(),
            updated_at: row.updated_at.and_utc(),
        }
    }
}

const ATTENDEE_COLUMNS: &str = r#"id, kbr_event_id, mail_subscriber_id, scan_count, headcount, created_at, updated_at"#;

pub async fn qr_scan(pool: &sqlx::PgPool, id: i64) -> Result<Option<EventAttendeeRow>, sqlx::Error> {
    let row = sqlx::query_as::<_, EventAttendeeRow>(
        &format!(
            r#"UPDATE kbr_event_attendees
               SET scan_count = COALESCE(scan_count, 0) + 1, updated_at = NOW()
               WHERE id = $1
               RETURNING {}"#,
            ATTENDEE_COLUMNS
        ),
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

pub async fn list_by_event(pool: &sqlx::PgPool, event_id: i32) -> Result<Vec<KbrEventAttendee>, sqlx::Error> {
    let rows = sqlx::query_as::<_, EventAttendeeRow>(
        &format!(
            r#"SELECT {} FROM kbr_event_attendees WHERE kbr_event_id = $1"#,
            ATTENDEE_COLUMNS
        ),
    )
    .bind(event_id)
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| r.into()).collect())
}

pub async fn create_one(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    kbr_event_id: i32,
    mail_subscriber_id: i32,
    headcount: Option<i32>,
    now: chrono::NaiveDateTime,
) -> Result<KbrEventAttendee, sqlx::Error> {
    let row = sqlx::query_as::<_, EventAttendeeRow>(
        &format!(
            r#"INSERT INTO kbr_event_attendees (kbr_event_id, mail_subscriber_id, scan_count, headcount, created_at, updated_at)
               VALUES ($1, $2, 0, $3, $4, $4)
               RETURNING {}"#,
            ATTENDEE_COLUMNS
        ),
    )
    .bind(kbr_event_id)
    .bind(mail_subscriber_id)
    .bind(headcount)
    .bind(now)
    .fetch_one(&mut **tx)
    .await?;

    Ok(row.into())
}
