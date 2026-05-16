//! Email job handlers.
//!
//! Each job looks up necessary data from the database, renders an email
//! template, and sends via the email service. Failures are logged but
//! do not propagate (jobs are fire-and-forget).

use ammonia::clean;
use image::Luma;
use percent_encoding::{percent_encode, AsciiSet, NON_ALPHANUMERIC};
use sqlx::FromRow;

use crate::app::AppState;
use crate::templates;

/// HTML-escape a string for safe template interpolation.
fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Strip characters that could inject email headers.
fn strip_newlines(s: &str) -> String {
    s.replace('\r', "").replace('\n', "")
}

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
    event_id: i64,
}

/// Send a QR code email to an event attendee.
///
/// Looks up the attendee's subscriber email and event details, generates
/// a QR code PNG, and sends the email.
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
        row.event_name.as_ref().map(|n| format!(" - {}", strip_newlines(n))).unwrap_or_default()
    );

    let qr_url = row.event_qr_encode_string.as_deref().unwrap_or("https://kushtybuckrecords.com");
    let qr_png = generate_qr_png(qr_url)?;

    let event_date = row
        .event_start_date
        .map(|dt| dt.format("%B %d").to_string())
        .unwrap_or_else(|| "TBD".to_string());

    let unsubscribe_url = build_unsubscribe_url(to)?;

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
        row.event_name.as_ref().map(|n| format!(" - {}", strip_newlines(n))).unwrap_or_default()
    );

    let unsubscribe_url = build_unsubscribe_url(to)?;

    let buttons = build_buttons_html(&row.event_ticket_url, &row.event_external_url);

    let html = templates::render(templates::text_copy_email::HTML, &[
        ("text_copy", &clean(text_copy)),
        ("buttons", &buttons),
        ("event_image", ""),
        ("unsubscribe_url", &unsubscribe_url),
    ]);

    send_email(state, to, &subject, &html).await?;

    tracing::info!(attendee_id, to, "Sent event update email");
    Ok(())
}

/// Generate a QR code PNG from the given URL string.
fn generate_qr_png(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let code = qrcode::QrCode::new(url)?;
    let img = code.render::<Luma<u8>>().min_dimensions(200, 200).build();
    let mut buf = Vec::new();
    image::write_buffer_with_format(
        &mut std::io::Cursor::new(&mut buf),
        img.as_raw(),
        img.width(),
        img.height(),
        image::ColorType::L8,
        image::ImageFormat::Png,
    )?;
    Ok(buf)
}

/// Build buttons HTML for ticket/external URLs.
fn build_buttons_html(ticket_url: &Option<String>, external_url: &Option<String>) -> String {
    let mut html = String::new();
    if let Some(url) = ticket_url {
        html.push_str(&format!(
            r#"<div style="margin: 30px 0;">
    <a href="{}" style="display: inline-block; padding: 12px 24px; background-color: #000000; color: #ffffff; text-decoration: none; border-radius: 6px; font-weight: bold; margin-right: 10px;">Buy Tickets</a>"#,
            url
        ));
    }
    if let Some(url) = external_url {
        if html.is_empty() {
            html.push_str(r#"<div style="margin: 30px 0;">"#);
        }
        html.push_str(&format!(
            r#"
    <a href="{}" style="display: inline-block; padding: 12px 24px; background-color: #000000; color: #ffffff; text-decoration: none; border-radius: 6px; font-weight: bold;">External Link</a>"#,
            url
        ));
    }
    if !html.is_empty() {
        html.push_str("</div>");
    }
    html
}

/// Build unsubscribe URL from APPLICATION_HOST env var.
fn build_unsubscribe_url(email: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let host = std::env::var("APPLICATION_HOST").unwrap_or_else(|_| "localhost:8181".to_string());
    // In production this would use a real token. For now, use email-based URL.
    Ok(format!("{}/v1/unsubscribe/{}", host, urlencoding(email)))
}

fn urlencoding(s: &str) -> String {
    // RFC 3986: encode everything except unreserved characters
    const FRAGMENT: AsciiSet = NON_ALPHANUMERIC.remove(b'-').remove(b'_').remove(b'.').remove(b'~');
    percent_encode(s.as_bytes(), &FRAGMENT).to_string()
}

/// Send a plain HTML email (no attachments).
async fn send_email(
    state: &AppState,
    to: &str,
    subject: &str,
    html: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match &state.email {
        Some(client) => client.send(to, subject, html, None).await.map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)) as Box<dyn std::error::Error + Send + Sync>),
        None => {
            tracing::warn!(to, subject, "Email not sent: SMTP not configured");
            Ok(())
        }
    }
}

/// Send an HTML email with a single attachment.
async fn send_email_with_attachment(
    state: &AppState,
    to: &str,
    subject: &str,
    html: &str,
    filename: &str,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match &state.email {
        Some(client) => client.send(to, subject, html, Some((filename, data))).await.map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::Other, e)) as Box<dyn std::error::Error + Send + Sync>),
        None => {
            tracing::warn!(to, subject, "Email not sent: SMTP not configured");
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_qr_png_produces_valid_png() {
        let png = generate_qr_png("https://example.com/test").unwrap();
        // PNG magic bytes: 89 50 4E 47 0D 0A 1A 0A
        assert_eq!(png[0], 0x89);
        assert_eq!(&png[1..4], b"PNG");
        assert!(png.len() > 100);
    }

    #[test]
    fn generate_qr_png_different_urls_produce_different_output() {
        let png1 = generate_qr_png("https://example.com/a").unwrap();
        let png2 = generate_qr_png("https://example.com/b").unwrap();
        assert_ne!(png1, png2);
    }

    #[test]
    fn build_buttons_html_ticket_only() {
        let html = build_buttons_html(&Some("https://tickets.com".to_string()), &None);
        assert!(html.contains("Buy Tickets"));
        assert!(html.contains("https://tickets.com"));
        assert!(!html.contains("External Link"));
    }

    #[test]
    fn build_buttons_html_external_only() {
        let html = build_buttons_html(&None, &Some("https://external.com".to_string()));
        assert!(html.contains("External Link"));
        assert!(html.contains("https://external.com"));
        assert!(!html.contains("Buy Tickets"));
    }

    #[test]
    fn build_buttons_html_both() {
        let html = build_buttons_html(
            &Some("https://tickets.com".to_string()),
            &Some("https://external.com".to_string()),
        );
        assert!(html.contains("Buy Tickets"));
        assert!(html.contains("External Link"));
    }

    #[test]
    fn build_buttons_html_none() {
        let html = build_buttons_html(&None, &None);
        assert!(html.is_empty());
    }

    #[test]
    fn urlencoding_basic() {
        assert_eq!(urlencoding("hello"), "hello");
        assert_eq!(urlencoding("hello world"), "hello%20world");
        assert_eq!(urlencoding("test@example.com"), "test%40example.com");
    }

    #[test]
    fn build_unsubscribe_url_default_host() {
        unsafe { std::env::remove_var("APPLICATION_HOST"); }
        let url = build_unsubscribe_url("test@example.com").unwrap();
        assert!(url.contains("localhost:8181"));
        assert!(url.contains("test%40example.com"));
    }
}
