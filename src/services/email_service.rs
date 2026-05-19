//! SMTP email service using lettre.
//!
//! Reads SMTP configuration from environment variables. Construction is
//! optional — returns `None` if `SMTP_HOST` is not set so the server can
//! start without email configured.

use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::AsyncSmtpTransport;
use lettre::AsyncTransport;
use lettre::Tokio1Executor;

use crate::error::AppError;

/// SMTP email client.
#[derive(Clone)]
pub struct EmailClient {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

impl EmailClient {
    /// Create from environment variables.
    ///
    /// Reads `SMTP_HOST`, `SMTP_PORT` (default 587), `SMTP_USER`,
    /// `SMTP_PASSWORD`, and `SMTP_FROM`. Returns `None` if `SMTP_HOST`
    /// is not set.
    pub fn new_from_env() -> Option<Self> {
        let host = std::env::var("SMTP_HOST").ok()?;
        let port: u16 = std::env::var("SMTP_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(587);
        let user = std::env::var("SMTP_USER").ok()?;
        let password = std::env::var("SMTP_PASSWORD").ok()?;
        let from = std::env::var("SMTP_FROM")
            .ok()
            .unwrap_or_else(|| "contact@kushtybuckrecords.com".to_string());

        Self::new(&host, port, &user, &password, &from)
    }

    /// Create with explicit configuration.
    pub fn new(
        host: &str,
        port: u16,
        user: &str,
        password: &str,
        from: &str,
    ) -> Option<Self> {
        let creds = Credentials::new(user.to_string(), password.to_string());
        let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(port)
            .credentials(creds)
            .build();

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
            let attach = Attachment::new(filename.to_string())
                .body(data.to_vec(), ContentType::parse("application/octet-stream").unwrap());

            let multipart = MultiPart::related()
                .singlepart(SinglePart::html(html.to_string()))
                .singlepart(attach);

            lettre::Message::builder()
                .from(self.from.parse().map_err(|e| {
                    AppError::Email(format!("Invalid from address: {}", e))
                })?)
                .to(to.parse().map_err(|e| {
                    AppError::Email(format!("Invalid to address: {}", e))
                })?)
                .subject(subject)
                .multipart(multipart)
                .map_err(|e| AppError::Email(format!("Failed to build email: {}", e)))?
        } else {
            lettre::Message::builder()
                .from(self.from.parse().map_err(|e| {
                    AppError::Email(format!("Invalid from address: {}", e))
                })?)
                .to(to.parse().map_err(|e| {
                    AppError::Email(format!("Invalid to address: {}", e))
                })?)
                .subject(subject)
                .header(ContentType::TEXT_HTML)
                .body(html.to_string())
                .map_err(|e| AppError::Email(format!("Failed to build email: {}", e)))?
        };

        self.transport
            .send(email)
            .await
            .map_err(|e| AppError::Email(format!("Failed to send email: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_from_env_returns_none_when_missing() {
        unsafe { std::env::remove_var("SMTP_HOST"); }
        assert!(EmailClient::new_from_env().is_none());
    }

    #[test]
    fn new_from_env_returns_some_when_set() {
        unsafe {
            std::env::set_var("SMTP_HOST", "smtp.example.com");
            std::env::set_var("SMTP_USER", "user@example.com");
            std::env::set_var("SMTP_PASSWORD", "pass123");
        }
        let client = EmailClient::new_from_env();
        assert!(client.is_some());
    }

    #[test]
    fn new_constructs_client() {
        let client = EmailClient::new(
            "smtp.example.com",
            587,
            "user@example.com",
            "pass123",
            "from@example.com",
        );
        assert!(client.is_some());
        assert_eq!(client.unwrap().from, "from@example.com");
    }

    #[test]
    fn client_is_clone() {
        let client =
            EmailClient::new("smtp.example.com", 587, "u", "p", "from@example.com").unwrap();
        let _clone = client.clone();
    }

    #[test]
    fn new_from_env_defaults_port() {
        unsafe {
            std::env::set_var("SMTP_HOST", "smtp.example.com");
            std::env::set_var("SMTP_USER", "user@example.com");
            std::env::set_var("SMTP_PASSWORD", "pass123");
            std::env::remove_var("SMTP_PORT");
        }
        let client = EmailClient::new_from_env();
        assert!(client.is_some());
    }

    #[test]
    fn new_from_env_custom_port() {
        unsafe {
            std::env::set_var("SMTP_HOST", "smtp.example.com");
            std::env::set_var("SMTP_PORT", "465");
            std::env::set_var("SMTP_USER", "user@example.com");
            std::env::set_var("SMTP_PASSWORD", "pass123");
        }
        let client = EmailClient::new_from_env();
        assert!(client.is_some());
    }
}
