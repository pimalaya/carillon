//! # Google Calendar wizard
//!
//! A Google account's calendar is watched over the Calendar API, which
//! takes a bearer token alone. The prompt collects it, usually as a
//! broker command since a token expires; the connection it opens is both
//! the test and where the calendars to pick from are listed.

use anyhow::{Result, bail};
use pimalaya_cli::{prompt, spinner::Spinner};
use pimalaya_config::secret::SecretResolver;

use crate::{
    config::{BearerAuthConfig, GcalConfig, GcalHookConfig, ItemHook, NotifyConfig},
    gcal,
    wizard::secret,
};

/// Configures Google Calendar for `email`, watching the calendar picked
/// among the account's own.
pub fn configure(account_name: &str, email: &str) -> Result<GcalConfig> {
    eprintln!(
        "Google Calendar takes an OAuth 2.0 token for {email}; issue and refresh it with a broker such as Ortie."
    );

    let key = format!("{account_name}-gcal");
    let token = secret::configure_token("Google Calendar access token", &key, true)?;
    let mut config = config(BearerAuthConfig { token });

    let spinner = Spinner::start("Testing Google Calendar connection");

    let listed = gcal::open(&config, &mut SecretResolver::new())
        .and_then(|mut client| gcal::calendars(&mut client));

    let calendars = match listed {
        Ok(calendars) => {
            spinner.success("Google Calendar connection succeeded");
            calendars
        }
        Err(err) => {
            spinner.failure("Google Calendar connection failed");
            return Err(err);
        }
    };

    if calendars.is_empty() {
        bail!("The Google account `{email}` holds no calendar to watch");
    }

    let names: Vec<String> = calendars.iter().map(|(_, name)| name.clone()).collect();
    let chosen = prompt::item("Calendar:", names, None)?;

    // NOTE: the id is what the account keeps, a summary being free to
    // change or to repeat across calendars.
    if let Some((id, _)) = calendars.into_iter().find(|(_, name)| *name == chosen) {
        config.calendar = id;
    }

    Ok(config)
}

/// Folds the credential into a block watching the primary calendar until
/// one is picked.
fn config(auth: BearerAuthConfig) -> GcalConfig {
    GcalConfig {
        calendar: String::from("primary"),
        tls: Default::default(),
        alpn: None,
        proxy: None,
        auth,
        watch: None,
        hook: GcalHookConfig {
            on_event_added: Some(ItemHook {
                notify: Some(NotifyConfig {
                    summary: String::from("New event in $calendar"),
                    body: String::from("$id"),
                }),
                cmd: None,
            }),
            ..Default::default()
        },
    }
}
