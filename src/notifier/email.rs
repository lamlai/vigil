// Vigil
//
// Microservices Status Page
// Copyright: 2018, Valerian Saliou <valerian@valeriansaliou.name>
// License: Mozilla Public License v2.0 (MPL v2.0)

use std::time::Duration;

use native_tls::TlsConnector;
use lettre::smtp::{ClientSecurity, SmtpTransportBuilder, SmtpTransport, ConnectionReuseParameters};
use lettre::smtp::authentication::Credentials;
use lettre::smtp::client::net::ClientTlsParameters;
use lettre::EmailTransport;
use lettre_email::EmailBuilder;

use super::generic::{DISPATCH_TIMEOUT_SECONDS, Notification, GenericNotifier};
use APP_CONF;

pub struct EmailNotifier;

impl GenericNotifier for EmailNotifier {
    fn dispatch(notification: &Notification) -> Result<(), bool> {
        if Self::is_enabled() == true {
            if let Some(ref email_config) = APP_CONF.notify.email {
                debug!(
                    "dispatch email notification for status: {:?} and replicas: {:?}",
                    notification.status, notification.replicas
                );

                // Build up the message text
                let mut message = String::new();

                message.push_str(&format!("{}\n", APP_CONF.branding.page_title));
                message.push_str("\n--\n\n");
                message.push_str(&format!("Status: {:?}\n", notification.status));
                message.push_str(&format!("Nodes: {}\n", notification.replicas.join(", ")));
                message.push_str(&format!("Date: {:?}\n", notification.time));
                message.push_str(&format!("URL: {}\n", APP_CONF.branding.page_url));

                // Build up the email
                let email_message = EmailBuilder::new()
                    .to(email_config.to.as_str())
                    .from((email_config.from.as_str(), APP_CONF.branding.page_title.as_str()))
                    .subject(format!(
                        "[{}] Status Changed ({:?})", &APP_CONF.branding.page_title,
                        notification.status
                    ))
                    .text(message)
                    .build()
                    .or(Err(true))?;

                // Deliver the message
                return acquire_transport(
                    &email_config.smtp_host,
                    email_config.smtp_port,
                    email_config.smtp_username.to_owned(),
                    email_config.smtp_password.to_owned(),
                    email_config.smtp_encrypt
                )
                    .map(|mut transport| transport.send(&email_message))
                    .and(Ok(()))
                    .or(Err(true));
            }
        }

        Err(false)
    }

    fn is_enabled() -> bool {
        APP_CONF.notify.email.is_some()
    }
}

fn acquire_transport(
    smtp_host: &str,
    smtp_port: u16,
    smtp_username: Option<String>,
    smtp_password: Option<String>,
    smtp_encrypt: bool
) -> Result<SmtpTransport, ()> {
    let mut security = ClientSecurity::None;

    if smtp_encrypt == true {
        if let Ok(connector_builder) = TlsConnector::builder() {
            if let Ok(connector) = connector_builder.build() {
                security = ClientSecurity::Required(ClientTlsParameters {
                    connector: connector,
                    domain: smtp_host.to_string(),
                });
            }
        }

        // Do not deliver email if TLS context cannot be acquired (prevents unencrypted emails \
        //   to be sent)
        if let ClientSecurity::None = security {
            error!("could not build smtp encrypted connector");

            return Err(());
        }
    }

    match SmtpTransportBuilder::new(format!("{}:{}", smtp_host, smtp_port), security) {
        Ok(transport) => {
            let mut transport_builder = transport
                .timeout(Some(Duration::from_secs(DISPATCH_TIMEOUT_SECONDS)))
                .connection_reuse(ConnectionReuseParameters::NoReuse);

            match (smtp_username, smtp_password) {
                (Some(smtp_username_value), Some(smtp_password_value)) => {
                    transport_builder = transport_builder.credentials(
                        Credentials::new(smtp_username_value, smtp_password_value)
                    );
                },
                _ => {},
            }

            Ok(transport_builder.build())
        }
        Err(err) => {
            error!("could not acquire smtp transport: {}", err);

            Err(())
        }
    }
}
