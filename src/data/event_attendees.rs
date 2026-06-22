use crate::models::kbr_event_attendee::KbrEventAttendee;

const ATTENDEE_COLUMNS: &str = r#"id, kbr_event_id, mail_subscriber_id, scan_count, headcount, created_at, updated_at"#;

pub async fn qr_scan(pool: &sqlx::PgPool, id: i64) -> Result<Option<KbrEventAttendee>, sqlx::Error> {
    sqlx::query_as::<_, KbrEventAttendee>(
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
    .await
}

pub async fn list_by_event(pool: &sqlx::PgPool, event_id: i32) -> Result<Vec<KbrEventAttendee>, sqlx::Error> {
    sqlx::query_as::<_, KbrEventAttendee>(
        &format!(
            r#"SELECT {} FROM kbr_event_attendees WHERE kbr_event_id = $1"#,
            ATTENDEE_COLUMNS
        ),
    )
    .bind(event_id)
    .fetch_all(pool)
    .await
}

pub async fn create_one(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    kbr_event_id: i32,
    mail_subscriber_id: i32,
    headcount: Option<i32>,
    now: chrono::NaiveDateTime,
) -> Result<KbrEventAttendee, sqlx::Error> {
    sqlx::query_as::<_, KbrEventAttendee>(
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
    .await
}
