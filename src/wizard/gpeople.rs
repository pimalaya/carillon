//! # Google People wizard
//!
//! A Google account's contacts are watched over the People API, which
//! takes a bearer token alone. The prompt collects it, usually as a
//! broker command since a token expires, and the connection test
//! resolves `myContacts`, the group holding every contact the account
//! owns.

use anyhow::Result;
use pimalaya_cli::spinner::Spinner;
use pimalaya_config::secret::SecretResolver;

use crate::{
    config::{BearerAuthConfig, GpeopleConfig, GpeopleHookConfig, ItemHook, NotifyConfig},
    gpeople,
    wizard::secret,
};

/// Configures Google People for `email`, watching every contact it owns,
/// and tests the connection.
pub fn configure(account_name: &str, email: &str) -> Result<GpeopleConfig> {
    eprintln!(
        "Google People takes an OAuth 2.0 token for {email}; issue and refresh it with a broker such as Ortie."
    );

    let key = format!("{account_name}-gpeople");
    let token = secret::configure_token("Google People access token", &key, true)?;
    let config = config(BearerAuthConfig { token });

    let spinner = Spinner::start("Testing Google People connection");

    match gpeople::probe(&config, &mut SecretResolver::new()) {
        Ok(()) => spinner.success("Google People connection succeeded"),
        Err(err) => {
            spinner.failure("Google People connection failed");
            return Err(err);
        }
    }

    Ok(config)
}

/// Folds the credential into a block watching every owned contact.
fn config(auth: BearerAuthConfig) -> GpeopleConfig {
    GpeopleConfig {
        addressbook: String::from("myContacts"),
        tls: Default::default(),
        alpn: None,
        proxy: None,
        auth,
        watch: None,
        hook: GpeopleHookConfig {
            on_card_added: Some(ItemHook {
                notify: Some(NotifyConfig {
                    summary: String::from("New contact in $addressbook"),
                    body: String::from("$id"),
                }),
                cmd: None,
            }),
            ..Default::default()
        },
    }
}
