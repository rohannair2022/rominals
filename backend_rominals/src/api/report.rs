use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use std::error::Error;
use std::io;

const GMAIL_SMTP_HOST: &str = "smtp.gmail.com";
const DEFAULT_REPORT_FROM_NAME: &str = "Rominals Terminal";

#[derive(Clone, Debug)]
pub struct ReportEmailConfig {
    pub smtp_username: String,
    smtp_app_password: String,
    pub recipient: String,
    pub from_name: String,
}

impl ReportEmailConfig {
    pub fn from_env() -> Result<Self, io::Error> {
        let smtp_username = required_env("ROMINALS_GMAIL_USER")?;
        let smtp_app_password = required_env("ROMINALS_GMAIL_APP_PASSWORD")?;
        let recipient = optional_env("ROMINALS_REPORT_TO")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| smtp_username.clone());
        let from_name = optional_env("ROMINALS_REPORT_FROM_NAME")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_REPORT_FROM_NAME.to_string());

        Ok(Self {
            smtp_username,
            smtp_app_password,
            recipient,
            from_name,
        })
    }
}

pub fn send_report_email(
    config: &ReportEmailConfig,
    subject: &str,
    body: &str,
) -> Result<(), Box<dyn Error>> {
    let from_email = config.smtp_username.parse()?;
    let to_email = config.recipient.parse()?;

    let message = Message::builder()
        .from(Mailbox::new(Some(config.from_name.clone()), from_email))
        .to(Mailbox::new(None, to_email))
        .subject(subject)
        .body(body.to_string())?;

    let credentials = Credentials::new(
        config.smtp_username.clone(),
        config.smtp_app_password.clone(),
    );
    let smtp = SmtpTransport::relay(GMAIL_SMTP_HOST)?
        .credentials(credentials)
        .build();

    smtp.send(&message)
        .map_err(|err| io::Error::other(format!("Gmail SMTP send failed: {err}")))?;
    Ok(())
}

fn required_env(name: &str) -> Result<String, io::Error> {
    std::env::var(name).map_err(|_| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("Missing required env var: {name}"),
        )
    })
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
}
