//! Event-related email jobs: attendee QR codes and text updates.

use sqlx::FromRow;

use crate::app::AppState;
use crate::templates;

use super::{build_buttons_html, build_unsubscribe_url, escape_html, strip_newlines, send_email, send_email_with_attachment, generate_qr_png};

#[derive(Debug, FromRow)]
struct AttendeeEmailRow {
    subscriber_email: String,
    subscriber_full_name: Option<String>,
    event_name: Option<String>,
    event_description: Option<String>,
    event_start_date: Option<chrono::NaiveDateTime>,
    event_qr_encode_string: Option<String>,
    event_ticket_url: Option<String>,
    event_external_url: Option<String>,
    _event_id: i64,
}

/// Send a QR code email to an event attendee.
pub async fn send_event_attendee_email(
    state: &AppState,
    attendee_id: i64,
    event_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let row = sqlx::query_as::<_, AttendeeEmailRow>(
        r#"SELECT
            ms.email AS subscriber_email,
            ms.full_name AS subscriber_full_name,
            ke.name AS event_name,
            ke.description AS event_description,
            ke.event_start_date,
            ke.qr_encode_string AS event_qr_encode_string,
            ke.ticket_url AS event_ticket_url,
            ke.external_url AS event_external_url,
            ke.id AS event_id
        FROM kbr_event_attendees kea
        JOIN mail_subscribers ms ON ms.id = kea.mail_subscriber_id
        JOIN kbr_events ke ON ke.id = kea.kbr_event_id
        WHERE kea.id = $1"#,
    )
    .bind(attendee_id)
    .fetch_optional(&state.db)
    .await?;

    let row = match row {
        Some(r) => r,
        None => {
            tracing::warn!(attendee_id, "Attendee not found for QR email");
            return Ok(());
        }
    };

    let to = &row.subscriber_email;
    let subject = format!(
        "QR Code for Event Attendance{}",
        row.event_name
            .as_ref()
            .map(|n| format!(" - {}", strip_newlines(n)))
            .unwrap_or_default()
    );

    let qr_url = row.event_qr_encode_string.as_deref().unwrap_or("https://kushtybuckrecords.com");
    let qr_png = generate_qr_png(qr_url)?;

    let event_date = row
        .event_start_date
        .map(|dt| dt.format("%B %d").to_string())
        .unwrap_or_else(|| "TBD".to_string());

    let unsubscribe_url = build_unsubscribe_url(&state.db, to).await?;

    let html = templates::render(templates::qr_code_email::HTML, &[
        ("event_name", &escape_html(row.event_name.as_deref().unwrap_or("Event"))),
        ("full_name", &escape_html(row.subscriber_full_name.as_deref().unwrap_or("Friend"))),
        (
            "event_description",
            &escape_html(row.event_description.as_deref().unwrap_or("")),
        ),
        ("event_date", &event_date),
        ("qr_image_cid", "qr_code"),
        ("event_image", ""),
        ("unsubscribe_url", &unsubscribe_url),
    ]);

    send_email_with_attachment(
        state,
        to,
        &subject,
        &html,
        "qr_code.png",
        &qr_png,
    )
    .await?;

    tracing::info!(attendee_id, event_id, to, "Sent event attendee QR email");
    Ok(())
}

/// Send a text update email to an event attendee.
pub async fn send_event_update_email(
    state: &AppState,
    attendee_id: i64,
    text_copy: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let row = sqlx::query_as::<_, AttendeeEmailRow>(
        r#"SELECT
            ms.email AS subscriber_email,
            ms.full_name AS subscriber_full_name,
            ke.name AS event_name,
            ke.description AS event_description,
            ke.event_start_date,
            ke.qr_encode_string AS event_qr_encode_string,
            ke.ticket_url AS event_ticket_url,
            ke.external_url AS event_external_url,
            ke.id AS event_id
        FROM kbr_event_attendees kea
        JOIN mail_subscribers ms ON ms.id = kea.mail_subscriber_id
        JOIN kbr_events ke ON ke.id = kea.kbr_event_id
        WHERE kea.id = $1"#,
    )
    .bind(attendee_id)
    .fetch_optional(&state.db)
    .await?;

    let row = match row {
        Some(r) => r,
        None => {
            tracing::warn!(attendee_id, "Attendee not found for text update email");
            return Ok(());
        }
    };

    let to = &row.subscriber_email;
    let subject = format!(
        "Updates about your upcoming event{}",
        row.event_name
            .as_ref()
            .map(|n| format!(" - {}", strip_newlines(n)))
            .unwrap_or_default()
    );

    let unsubscribe_url = build_unsubscribe_url(&state.db, to).await?;

    let buttons = build_buttons_html(&row.event_ticket_url, &row.event_external_url);

    let html = templates::render(templates::text_copy_email::HTML, &[
        ("text_copy", &ammonia::clean(text_copy)),
        ("buttons", &buttons),
        ("event_image", ""),
        ("unsubscribe_url", &unsubscribe_url),
    ]);

    send_email(state, to, &subject, &html).await?;

    tracing::info!(attendee_id, to, "Sent event update email");
    Ok(())
}
