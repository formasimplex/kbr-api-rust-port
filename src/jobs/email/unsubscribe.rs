//! Unsubscribe confirmation email job.

use crate::app::AppState;
use crate::templates;

use super::{escape_html, send_email};

/// Send an unsubscribe confirmation email.
pub async fn send_unsubscribe_email(
    state: &AppState,
    email: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let subscriber: Option<(String,)> = sqlx::query_as(
        r#"SELECT full_name FROM mail_subscribers WHERE email = $1 AND unsubscribed_at IS NULL"#,
    )
    .bind(email)
    .fetch_optional(&state.db)
    .await?;

    let full_name = escape_html(
        &subscriber
            .map(|s| s.0)
            .unwrap_or_else(|| "Friend".to_string())
    );

    let token = crate::models::mail_subscriber::MailSubscriber::generate_unsubscribe_token();
    let expires_at = chrono::Utc::now() + chrono::TimeDelta::hours(24);

    sqlx::query(
        r#"UPDATE mail_subscribers SET unsubscribe_token = $1, unsubscribe_token_expires_at = $2, updated_at = NOW()
           WHERE email = $3 AND unsubscribed_at IS NULL"#,
    )
    .bind(&token)
    .bind(expires_at.naive_utc())
    .bind(email)
    .execute(&state.db)
    .await?;

    let host = std::env::var("APPLICATION_HOST").unwrap_or_else(|_| "localhost:8181".to_string());
    let unsubscribe_url = format!("{}/v1/unsubscribe/{}", host, token);

    let subject = "Confirm your unsubscribe request";

    let html = templates::render(templates::unsubscribe_email::HTML, &[
        ("full_name", &full_name),
        ("unsubscribe_url", &unsubscribe_url),
    ]);

    send_email(state, email, subject, &html).await?;

    tracing::info!(email, "Sent unsubscribe confirmation email");
    Ok(())
}
