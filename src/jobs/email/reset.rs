//! Password reset email job.

use sqlx::FromRow;

use crate::app::AppState;
use crate::templates;

use super::{escape_html, send_email};

#[derive(Debug, FromRow)]
struct ResetTriggerEmailRow {
    _id: i64,
    user_id: Option<i32>,
    email: Option<String>,
    token: String,
}

/// Send a password reset email.
pub async fn send_reset_trigger_email(
    state: &AppState,
    reset_trigger_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let row = sqlx::query_as::<_, ResetTriggerEmailRow>(
        r"SELECT rt.id, rt.user_id, u.email, rt.token
           FROM reset_triggers rt
           LEFT JOIN users u ON u.id = rt.user_id
           WHERE rt.id = $1"
    )
    .bind(reset_trigger_id)
    .fetch_optional(&state.db)
    .await?;

    let row = match row {
        Some(r) => r,
        None => {
            tracing::warn!(reset_trigger_id, "ResetTrigger not found for email");
            return Ok(());
        }
    };

    if row.user_id.is_none() {
        tracing::info!(reset_trigger_id, "Reset trigger has no associated user, skipping email");
        return Ok(());
    }

    let to = row.email.as_deref().unwrap_or("");
    if to.is_empty() {
        tracing::warn!(reset_trigger_id, "Reset trigger has no email to send to");
        return Ok(());
    }

    let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let confirmation_url = format!("{}/reset_password/{}", frontend_url, row.token);

    let full_name = escape_html("Friend");

    let subject = "Reset Your Password";

    let html = templates::render(templates::reset_trigger_email::HTML, &[
        ("full_name", &full_name),
        ("confirmation_url", &confirmation_url),
    ]);

    send_email(state, to, &subject, &html).await?;

    tracing::info!(reset_trigger_id, to, "Sent password reset email");
    Ok(())
}
