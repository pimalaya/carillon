//! # Gmail wizard
//!
//! A Google account's mail is watched over the Gmail API, which takes a
//! bearer token alone. The prompt collects it, usually as a broker
//! command since a token expires, and the connection test resolves the
//! inbox label the generated account watches.

use anyhow::Result;
use pimalaya_cli::spinner::Spinner;
use pimalaya_config::secret::SecretResolver;

use crate::{
    config::{BearerAuthConfig, GmailConfig, GmailHookConfig, ItemHook, NotifyConfig},
    gmail,
    wizard::secret,
};

/// Configures Gmail for `email`, watching its inbox, and tests the
/// connection.
pub fn configure(account_name: &str, email: &str) -> Result<GmailConfig> {
    eprintln!(
        "Gmail takes an OAuth 2.0 token for {email}; issue and refresh it with a broker such as Ortie."
    );

    let key = format!("{account_name}-gmail");
    let token = secret::configure_token("Gmail access token", &key, true)?;
    let config = config(BearerAuthConfig { token });

    let spinner = Spinner::start("Testing Gmail connection");

    match gmail::probe(&config, &mut SecretResolver::new()) {
        Ok(()) => spinner.success("Gmail connection succeeded"),
        Err(err) => {
            spinner.failure("Gmail connection failed");
            return Err(err);
        }
    }

    Ok(config)
}

/// Folds the credential into a block watching the inbox label.
fn config(auth: BearerAuthConfig) -> GmailConfig {
    GmailConfig {
        mailbox: String::from("INBOX"),
        user_id: None,
        tls: Default::default(),
        alpn: None,
        proxy: None,
        auth,
        watch: None,
        hook: GmailHookConfig {
            on_message_added: Some(ItemHook {
                notify: Some(NotifyConfig {
                    summary: String::from("New mail in $mailbox from $sender"),
                    body: String::from("$subject"),
                }),
                cmd: None,
            }),
            ..Default::default()
        },
    }
}
