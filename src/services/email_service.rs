//! SMTP email service using lettre.
//!
//! Reads SMTP configuration from environment variables. Construction is
//! optional — returns `None` if `SMTP_HOST` is not set so the server can
//! start without email configured.

use crate::error::AppError;
use lettre::AsyncTransport;
use lettre::Tokio1Executor;
use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart, SinglePart};
use lettre::transport::smtp::AsyncSmtpTransport;
use lettre::transport::smtp::authentication::Credentials;

/// SMTP email client.
#[derive(Clone)]
pub struct EmailClient {
    transport: AsyncSmtpTransport<Tokio1Executor>,
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
        let creds = Credentials::new(user.to_string(), password.to_string());

        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .ok()?
            .port(587)
            .credentials(creds)
            .build();

        tracing::info!(
            smtp_host = host,
            smtp_user = user,
            tls = "STARTTLS",
            "SMTP email client configured"
        );

        Some(Self {
            transport,
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
        let email = if let Some((filename, data)) = attachment {
            let attach = Attachment::new(filename.to_string()).body(
                data.to_vec(),
                ContentType::parse("application/octet-stream").unwrap(),
            );

            let multipart = MultiPart::related()
                .singlepart(SinglePart::html(html.to_string()))
                .singlepart(attach);

            lettre::Message::builder()
                .from(
                    self.from
                        .parse()
                        .map_err(|e| AppError::Email(format!("Invalid from address: {}", e)))?,
                )
                .to(to
                    .parse()
                    .map_err(|e| AppError::Email(format!("Invalid to address: {}", e)))?)
                .subject(subject)
                .multipart(multipart)
                .map_err(|e| AppError::Email(format!("Failed to build email: {}", e)))?
        } else {
            let html_part = SinglePart::html(html.to_string());

            lettre::Message::builder()
                .from(
                    self.from
                        .parse()
                        .map_err(|e| AppError::Email(format!("Invalid from address: {}", e)))?,
                )
                .to(to
                    .parse()
                    .map_err(|e| AppError::Email(format!("Invalid to address: {}", e)))?)
                .subject(subject)
                .singlepart(html_part)
                .map_err(|e| AppError::Email(format!("Failed to build email: {}", e)))?
        };

        self.transport.send(email).await.map_err(|e| {
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
