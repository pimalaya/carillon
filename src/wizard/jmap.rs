//! # JMAP wizard
//!
//! A discovery entry pins the session endpoint, so
//! [`configure_discovered`] picks the HTTP authentication scheme, prompts
//! its credentials and opens the session.
//!
//! That session is both the connection test and where the watch method is
//! decided: a server publishing no event-source URL cannot be pushed to
//! and gets an explicit poll, every other one being left alone, an unset
//! `jmap.watch` already meaning the held event stream.

use anyhow::{Result, bail};
use pimalaya_cli::{prompt, spinner::Spinner};

use crate::{
    config::{
        ItemHook, JmapAuthConfig, JmapConfig, JmapHookConfig, JmapWatchConfig, NotifyConfig,
        PollWatchConfig,
    },
    jmap,
    wizard::{
        search::{AuthCaps, Discovered, DiscoveredKind},
        secret,
    },
};

const BASIC: &str = "Basic (username + password)";
const BEARER: &str = "Bearer (API token)";

/// The interval an account falls back to when its server publishes no
/// event-source URL, which the polling watch takes anyway.
const POLL_INTERVAL: u64 = 30;

/// Configures JMAP from a discovered entry, whose endpoint is pinned.
///
/// The HTTP authentication scheme is picked among the advertised ones,
/// skipped when only one qualifies, then its credentials prompted. The
/// connection is tested here, so the caller has nothing left to prove.
pub fn configure_discovered(
    account_name: &str,
    email: &str,
    discovered: &Discovered,
) -> Result<JmapConfig> {
    let DiscoveredKind::Jmap(server) = &discovered.kind else {
        bail!("Expected a JMAP configuration");
    };

    let auth = prompt_auth(
        account_name,
        discovered.login_default(email).as_deref(),
        discovered.auth,
    )?;
    let mut config = config(server.clone(), auth);

    let spinner = Spinner::start("Testing JMAP connection");

    let client = match jmap::open(&config) {
        Ok((client, _url)) => client,
        Err(err) => {
            spinner.failure("JMAP connection failed");
            return Err(err);
        }
    };

    spinner.success("JMAP connection succeeded");

    // NOTE: an unset `watch` is already the event stream, so only a
    // session with nothing to stream from leaves a line in the account.
    let pushes = client
        .session()
        .is_some_and(|session| !session.event_source_url.is_empty());

    if !pushes {
        config.watch = Some(JmapWatchConfig::Poll(PollWatchConfig {
            interval: Some(POLL_INTERVAL),
        }));
    }

    Ok(config)
}

/// Prompts the HTTP authentication scheme from `caps`, both offered when
/// none was advertised, then its credentials.
///
/// The Bearer flow shows the OAuth brokers only when a grant was
/// advertised.
fn prompt_auth(
    account_name: &str,
    login_hint: Option<&str>,
    caps: AuthCaps,
) -> Result<JmapAuthConfig> {
    let mut schemes = Vec::new();

    if caps.basic || !caps.any() {
        schemes.push(BASIC);
    }

    if caps.token() || !caps.any() {
        schemes.push(BEARER);
    }

    let scheme = if schemes.len() == 1 {
        schemes[0]
    } else {
        prompt::item("JMAP authentication:", schemes, None)?
    };

    let key = format!("{account_name}-jmap");

    Ok(match scheme {
        BASIC => JmapAuthConfig::Basic {
            username: prompt::text("Login:", login_hint)?,
            password: secret::configure_password("JMAP password", &key)?,
        },
        _ => JmapAuthConfig::Bearer {
            token: secret::configure_token("JMAP API token", &key, caps.oauth || !caps.any())?,
        },
    })
}

/// Folds the endpoint and credentials into a block watching the inbox,
/// the mailbox a first account watches and the one every server has.
fn config(server: String, auth: JmapAuthConfig) -> JmapConfig {
    JmapConfig {
        mailbox: String::from("INBOX"),
        server,
        tls: Default::default(),
        auth,
        watch: None,
        hook: hook(),
    }
}

/// The hook a generated account fires: a notification on arrival, which
/// is what someone watching a mailbox came for.
///
/// JMAP reads an arrival's envelope from the request its round already
/// makes, so the notification may name it.
fn hook() -> JmapHookConfig {
    JmapHookConfig {
        on_message_added: Some(ItemHook {
            notify: Some(NotifyConfig {
                summary: String::from("New mail in $mailbox from $sender"),
                body: String::from("$subject"),
            }),
            cmd: None,
        }),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_generated_account_watches_the_inbox_and_notifies_on_arrival() {
        let config = config(
            String::from("https://api.example.org/jmap/session"),
            JmapAuthConfig::Bearer {
                token: pimalaya_config::secret::Secret::Raw("token".into()),
            },
        );

        assert_eq!(config.mailbox, "INBOX");
        assert!(config.watch.is_none());
        assert!(config.hook.on_message_added.is_some());
    }
}
