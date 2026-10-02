//! # Microsoft Graph wizard
//!
//! A Microsoft account is watched over the Graph API, which takes a
//! bearer token alone. The prompt collects it, usually as a broker
//! command since a token expires, and the connection test resolves the
//! inbox the generated account watches.

use anyhow::Result;
use pimalaya_cli::spinner::Spinner;
use pimalaya_config::secret::SecretResolver;

use crate::{
    config::{BearerAuthConfig, DomainsHookConfig, ItemHook, MsgraphConfig, NotifyConfig},
    msgraph,
    wizard::secret,
};

/// Configures Microsoft Graph for `email`, watching its inbox, and tests
/// the connection.
pub fn configure(account_name: &str, email: &str) -> Result<MsgraphConfig> {
    eprintln!(
        "Microsoft Graph takes an OAuth 2.0 token for {email}; issue and refresh it with a broker such as Ortie."
    );

    let key = format!("{account_name}-msgraph");
    let token = secret::configure_token("Microsoft Graph access token", &key, true)?;
    let config = config(BearerAuthConfig { token });

    let spinner = Spinner::start("Testing Microsoft Graph connection");

    match msgraph::probe(&config, &mut SecretResolver::new()) {
        Ok(()) => spinner.success("Microsoft Graph connection succeeded"),
        Err(err) => {
            spinner.failure("Microsoft Graph connection failed");
            return Err(err);
        }
    }

    Ok(config)
}

/// Folds the credential into a block watching the inbox, through its
/// well-known name since the display name is localized.
fn config(auth: BearerAuthConfig) -> MsgraphConfig {
    MsgraphConfig {
        mailbox: Some(String::from("inbox")),
        addressbook: None,
        calendar: None,
        user_id: None,
        tls: Default::default(),
        alpn: None,
        proxy: None,
        auth,
        watch: None,
        hook: DomainsHookConfig {
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
