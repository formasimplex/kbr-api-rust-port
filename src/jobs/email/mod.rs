//! Email job utilities and module re-exports.

mod event;
mod reset;
mod sign_up;
mod unsubscribe;

pub use event::{send_event_attendee_email, send_event_update_email};
pub use reset::send_reset_trigger_email;
pub use sign_up::{send_prospect_welcome_email, send_sign_up_trigger_email};
pub use unsubscribe::send_unsubscribe_email;

use email_address::EmailAddress;
use image::Luma;

/// HTML-escape a string for safe template interpolation.
pub(crate) fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Strip characters that could inject email headers.
pub(crate) fn strip_newlines(s: &str) -> String {
    s.replace(['\r', '\n'], "")
}

/// Validate an email address using RFC 5322 parsing. Returns an error
/// if the address is invalid or contains CRLF characters.
pub(crate) fn validate_email_address(email: &str) -> Result<(), String> {
    let sanitized = strip_newlines(email);
    if sanitized.is_empty() {
        return Err("Email address is empty".to_string());
    }
    if sanitized.contains('\r') || sanitized.contains('\n') {
        return Err("Email address contains forbidden characters".to_string());
    }
    EmailAddress::is_valid(&sanitized).then_some(()).ok_or_else(|| {
        format!("Invalid email address format: {}", email)
    })
}

/// Generate a QR code PNG from the given URL string.
pub(crate) fn generate_qr_png(
    url: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
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
pub(crate) fn build_buttons_html(ticket_url: &Option<String>, external_url: &Option<String>) -> String {
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
/// Looks up or generates a token for the given email address.
pub(crate) async fn build_unsubscribe_url(
    pool: &sqlx::PgPool,
    email: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let host = std::env::var("APPLICATION_HOST").unwrap_or_else(|_| "localhost:8181".to_string());

    let existing: Option<(String,)> = sqlx::query_as(
        r#"SELECT unsubscribe_token FROM mail_subscribers WHERE email = $1 AND unsubscribed_at IS NULL"#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    let token = match existing {
        Some((t,)) if !t.is_empty() => t,
        _ => {
            let token = crate::models::mail_subscriber::MailSubscriber::generate_unsubscribe_token();
            let expires_at = chrono::Utc::now() + chrono::TimeDelta::hours(24);
            sqlx::query(
                r#"UPDATE mail_subscribers SET unsubscribe_token = $1, unsubscribe_token_expires_at = $2, updated_at = NOW()
                   WHERE email = $3 AND unsubscribed_at IS NULL"#,
            )
            .bind(&token)
            .bind(expires_at.naive_utc())
            .bind(email)
            .execute(pool)
            .await?;
            token
        }
    };

    Ok(format!("{}/v1/unsubscribe/{}", host, token))
}

/// Send a plain HTML email (no attachments).
pub(crate) async fn send_email(
    state: &crate::app::AppState,
    to: &str,
    subject: &str,
    html: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    validate_email_address(to).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
    let to = strip_newlines(to);
    let subject = strip_newlines(subject);

    match &state.email {
        Some(client) => client
            .send(&to, &subject, html, None)
            .await
            .map_err(|e| Box::new(std::io::Error::other(e)) as Box<dyn std::error::Error + Send + Sync>),
        None => {
            tracing::warn!(to = %to, subject = %subject, "Email not sent: SMTP not configured");
            Ok(())
        }
    }
}

/// Send an HTML email with a single attachment.
pub(crate) async fn send_email_with_attachment(
    state: &crate::app::AppState,
    to: &str,
    subject: &str,
    html: &str,
    filename: &str,
    data: &[u8],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    validate_email_address(to).map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.into() })?;
    let to = strip_newlines(to);
    let subject = strip_newlines(subject);

    match &state.email {
        Some(client) => client
            .send(&to, &subject, html, Some((filename, data)))
            .await
            .map_err(|e| Box::new(std::io::Error::other(e)) as Box<dyn std::error::Error + Send + Sync>),
        None => {
            tracing::warn!(to = %to, subject = %subject, "Email not sent: SMTP not configured");
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
    fn escape_html_basic() {
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("a & b"), "a &amp; b");
    }

    #[test]
    fn strip_newlines_removes_cr_lf() {
        assert_eq!(strip_newlines("a\r\nb"), "ab");
    }

    #[test]
    fn validate_email_address_valid() {
        assert!(validate_email_address("user@example.com").is_ok());
        assert!(validate_email_address("test.user+tag@subdomain.example.co.uk").is_ok());
    }

    #[test]
    fn validate_email_address_invalid_empty() {
        assert!(validate_email_address("").is_err());
    }

    #[test]
    fn validate_email_address_invalid_no_at() {
        assert!(validate_email_address("userexample.com").is_err());
    }

    #[test]
    fn validate_email_address_strips_cr_lf() {
        assert!(validate_email_address("user\r\n@example.com").is_ok());
    }
}
