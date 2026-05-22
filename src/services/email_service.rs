//! SMTP email service using mail-send.
//!
//! Reads SMTP configuration from environment variables. Construction is
//! optional — returns `None` if `SMTP_HOST` is not set so the server can
//! start without email configured.

use crate::error::AppError;
use mail_send::mail_builder::MessageBuilder;
use mail_send::SmtpClientBuilder;

/// SMTP email client.
///
/// Stores the connection builder (which is `Clone`) and opens a fresh
/// connection for every send.  This is fine for a background-job queue
/// that sends emails sequentially.
#[derive(Clone)]
pub struct EmailClient {
    builder: SmtpClientBuilder<String>,
    from: String,
}

impl EmailClient {
    /// Create from environment variables.
    ///
    /// Reads `SMTP_HOST`, `SMTP_USER`, `SMTP_PASSWORD`, and `SMTP_FROM`.
    /// Returns `None` if `SMTP_HOST` is not set.
    pub fn new_from_env() -> Option<Self> {
        let host = std::env::var("SMTP_HOST")
            .ok()?
            .trim_matches('\'')
            .trim()
            .to_string();
        let user = std::env::var("SMTP_USER")
            .ok()?
            .trim_matches('\'')
            .trim()
            .to_string();
        let password = std::env::var("SMTP_PASSWORD")
            .ok()?
            .trim_matches('\'')
            .trim()
            .to_string();
        let from = std::env::var("SMTP_FROM")
            .ok()
            .unwrap_or_else(|| "contact@kushtybuckrecords.com".to_string())
            .trim_matches('\'')
            .trim()
            .to_string();

        Self::new(&host, &user, &password, &from)
    }

    /// Create with explicit configuration.
    pub fn new(host: &str, user: &str, password: &str, from: &str) -> Option<Self> {
        let builder = SmtpClientBuilder::new(host.to_string(), 587)
            .ok()?
            .implicit_tls(false)
            .credentials((user.to_string(), password.to_string()));

        tracing::info!(
            smtp_host = host,
            smtp_user = user,
            tls = "STARTTLS",
            "SMTP email client configured"
        );

        Some(Self {
            builder,
            from: from.to_string(),
        })
    }

    /// Send an HTML email.
    ///
    /// # Arguments
    ///
    /// * `to` — Recipient email address
    /// * `subject` — Email subject line
    /// * `html` — HTML body content
    /// * `attachment` — Optional attachment as (filename, data)
    pub async fn send(
        &self,
        to: &str,
        subject: &str,
        html: &str,
        attachment: Option<(&str, &[u8])>,
    ) -> Result<(), AppError> {
        let mut message = MessageBuilder::new()
            .from(self.from.as_str())
            .to(to)
            .subject(subject)
            .html_body(html);

        if let Some((filename, data)) = attachment {
            message = message.attachment("application/octet-stream", filename, data);
        }

        self.builder
            .connect()
            .await
            .map_err(|e| {
                tracing::error!(
                    to = to,
                    subject = subject,
                    from = %self.from,
                    "SMTP connect failed: {}", e
                );
                AppError::Email(format!("Failed to connect to SMTP server: {}", e))
            })?
            .send(message)
            .await
            .map_err(|e| {
                tracing::error!(
                    to = to,
                    subject = subject,
                    from = %self.from,
                    "SMTP send failed: {}", e
                );
                AppError::Email(format!("Failed to send email: {}", e))
            })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_is_clone() {
        let client = EmailClient::new("smtp.example.com", "u", "p", "from@example.com").unwrap();
        let _clone = client.clone();
    }
}
