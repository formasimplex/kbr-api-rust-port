//! Sign-up related email jobs: prospect welcome and sign-up trigger.

use sqlx::FromRow;

use crate::app::AppState;
use crate::templates;

use super::{escape_html, send_email};

#[derive(Debug, FromRow)]
struct ProspectEmailRow {
    email: String,
    artist_name: Option<String>,
}

/// Send a prospect welcome email to a newly signed-up artist.
pub async fn send_prospect_welcome_email(
    state: &AppState,
    user_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let row = sqlx::query_as::<_, ProspectEmailRow>(
        r"SELECT u.email, a.name AS artist_name
           FROM users u
           LEFT JOIN artists a ON a.user_id = u.id
           WHERE u.id = $1"
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    let row = match row {
        Some(r) => r,
        None => {
            tracing::warn!(user_id, "User not found for prospect welcome email");
            return Ok(());
        }
    };

    let full_name = escape_html(
        row.artist_name
            .as_deref()
            .unwrap_or("Friend"),
    );

    let subject = "Welcome to Kushty Buck Records";

    let html = templates::render(templates::prospect_welcome_email::HTML, &[
        ("full_name", &full_name),
    ]);

    send_email(state, &row.email, &subject, &html).await?;

    tracing::info!(user_id, to = row.email, "Sent prospect welcome email");
    Ok(())
}

#[derive(Debug, FromRow)]
struct SignUpTriggerEmailRow {
    id: i64,
    email: String,
    token: String,
    role: String,
}

/// Send a sign-up trigger confirmation email.
pub async fn send_sign_up_trigger_email(
    state: &AppState,
    sign_up_trigger_id: i64,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let row = sqlx::query_as::<_, SignUpTriggerEmailRow>(
        r"SELECT id, email, token, role FROM sign_up_triggers WHERE id = $1"
    )
    .bind(sign_up_trigger_id)
    .fetch_optional(&state.db)
    .await?;

    let row = match row {
        Some(r) => r,
        None => {
            tracing::warn!(sign_up_trigger_id, "SignUpTrigger not found for email");
            return Ok(());
        }
    };

    let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let confirm_url = format!("{}/sign_up/{}", frontend_url, row.token);
    let role = escape_html(&row.role);

    let subject = format!("Confirm Your {} Account", role);

    let html = templates::render(templates::sign_up_trigger_email::HTML, &[
        ("role", &role),
        ("confirm_url", &confirm_url),
    ]);

    send_email(state, &row.email, &subject, &html).await?;

    tracing::info!(sign_up_trigger_id, to = row.email, role, "Sent sign-up trigger email");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prospect_email_row_struct() {
        let row = ProspectEmailRow {
            email: "test@example.com".to_string(),
            artist_name: Some("Test Artist".to_string()),
        };
        assert_eq!(row.email, "test@example.com");
        assert_eq!(row.artist_name.as_deref(), Some("Test Artist"));
    }

    #[test]
    fn prospect_email_row_no_artist_name() {
        let row = ProspectEmailRow {
            email: "test@example.com".to_string(),
            artist_name: None,
        };
        assert_eq!(row.artist_name, None);
    }
}
