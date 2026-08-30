//! # IMAP wizard
//!
//! A discovery entry pins the endpoint, so [`configure_discovered`] reads
//! the mechanisms the server advertises, picks one, prompts its
//! credentials and opens the session.
//!
//! That session is both the connection test and where the watch method is
//! decided: a server not advertising IDLE gets an explicit poll, one
//! advertising it is left alone, an unset `imap.watch` already meaning
//! IDLE.
//!
//! The mechanism menu comes from the server rather than from discovery,
//! which reports whether a provider takes a password or an OAuth token
//! and never which mechanism carries it. Discovery's list is the
//! fallback, for a server the probe could not reach.

use anyhow::{Result, bail};
use io_imap::{has_imap_capability, rfc3501::capability::available_auth_mechanisms};
use io_pim_discovery::compose::config::DiscoverySecurity;
use io_sasl::mechanism::SaslMechanism;
use log::warn;
use pimalaya_cli::{prompt, spinner::Spinner};
use pimalaya_config::secret::SecretResolver;

use crate::{
    config::{
        ImapConfig, ImapHookConfig, ImapWatchConfig, ItemHook, NotifyConfig, PollWatchConfig,
        SaslAnonymousConfig, SaslConfig, SaslLoginConfig, SaslOauthbearerConfig, SaslPlainConfig,
        SaslScramSha256Config, SaslXoauth2Config,
    },
    imap,
    wizard::{
        search::{AuthCaps, Discovered, DiscoveredKind, TcpEndpoint},
        secret,
    },
};

// NOTE: the mechanisms split by credential kind, a password family and a
// token family, ANONYMOUS carrying none.
const PLAIN: &str = "PLAIN (username + password)";
const LOGIN: &str = "LOGIN (username + password)";
const SCRAM_SHA_256: &str = "SCRAM-SHA-256 (username + password)";
const ANONYMOUS: &str = "ANONYMOUS (no credentials)";
const OAUTHBEARER: &str = "OAUTHBEARER (username + API token)";
const XOAUTH2: &str = "XOAUTH2 (username + API token)";

/// The interval an account falls back to when its server cannot hold an
/// IDLE, which io-imap's own polling watch takes anyway.
const POLL_INTERVAL: u64 = 60;

/// Configures IMAP from a discovered entry.
///
/// The mechanisms the server advertises are read, one and its credentials
/// picked, the session opened, then IDLE kept or a poll written. The
/// connection is tested here, so the caller has nothing left to prove.
pub fn configure_discovered(
    account_name: &str,
    email: &str,
    discovered: &Discovered,
) -> Result<ImapConfig> {
    let DiscoveredKind::Imap(endpoint) = &discovered.kind else {
        bail!("Expected an IMAP configuration");
    };

    let login_hint = discovered.login_default(email);
    let mut config = config(endpoint, None);

    // NOTE: the server is the only authority on what it accepts, discovery
    // reporting a provider's password and OAuth policy and not its
    // mechanism list, so it would offer SCRAM-SHA-256 to a Gmail that has
    // never implemented it. A failed probe falls back to it all the same,
    // rather than leaving the menu empty.
    let probed = probe_mechanisms(&config);
    let mechanism = prompt_mechanism(discovered.auth, probed.as_deref())?;

    config.sasl = Some(build_sasl(
        mechanism,
        account_name,
        login_hint.as_deref(),
        discovered.auth,
    )?);

    let spinner = Spinner::start("Testing IMAP connection");

    let capabilities = match imap::open(&config, &mut SecretResolver::new()) {
        Ok((_client, capabilities)) => capabilities,
        Err(err) => {
            spinner.failure("IMAP connection failed");
            return Err(err);
        }
    };

    spinner.success("IMAP connection succeeded");

    // NOTE: an unset `watch` is already IDLE, so only a server that
    // cannot hold one leaves a line in the account.
    if !has_imap_capability!(capabilities, Idle) {
        config.watch = Some(ImapWatchConfig::Poll(PollWatchConfig {
            interval: Some(POLL_INTERVAL),
        }));
    }

    Ok(config)
}

/// Opens an unauthenticated session to read the server's `CAPABILITY`,
/// most preferred mechanism first and LOGIN last.
///
/// [`None`] when the probe failed or advertised nothing usable, the error
/// being logged and never surfaced: the menu falls back to what discovery
/// reported rather than stopping.
fn probe_mechanisms(config: &ImapConfig) -> Option<Vec<SaslMechanism>> {
    let spinner = Spinner::start("Reading IMAP capabilities");

    let probed = imap::open(config, &mut SecretResolver::new()).map(|(_client, capabilities)| {
        available_auth_mechanisms(&capabilities)
            .into_iter()
            .filter(is_expressible)
            .collect::<Vec<_>>()
    });

    match probed {
        Ok(mechanisms) if !mechanisms.is_empty() => {
            spinner.success(format!(
                "Server advertises {} mechanism(s)",
                mechanisms.len()
            ));
            Some(mechanisms)
        }
        Ok(_) => {
            spinner.failure("Server advertises no mechanism this build can run");
            None
        }
        Err(err) => {
            spinner.failure("Could not read the IMAP capabilities");
            warn!("could not probe imap capabilities, offering every mechanism: {err:#}");
            None
        }
    }
}

/// Prompts the authentication mechanism, among what the server advertised
/// or, failing that, what discovery keys.
///
/// A single candidate is selected without prompting.
fn prompt_mechanism(caps: AuthCaps, probed: Option<&[SaslMechanism]>) -> Result<SaslMechanism> {
    let mechanisms = offered(caps, probed);
    let labels: Vec<&str> = mechanisms.iter().map(mechanism_label).collect();

    let label = if labels.len() == 1 {
        labels[0]
    } else {
        prompt::item("SASL mechanism:", labels, None)?
    };

    // NOTE: labels are unique, so the chosen one maps back to exactly one
    // mechanism.
    Ok(mechanisms
        .into_iter()
        .find(|mechanism| mechanism_label(mechanism) == label)
        .expect("chosen label matches a mechanism"))
}

/// Prompts the credentials for `mechanism` and builds its SASL config.
///
/// ANONYMOUS carries no login; every other mechanism needs one, plus a
/// password or an API token, whichever its family takes.
fn build_sasl(
    mechanism: SaslMechanism,
    account_name: &str,
    login_hint: Option<&str>,
    caps: AuthCaps,
) -> Result<SaslConfig> {
    if let SaslMechanism::Anonymous = mechanism {
        let message = prompt::text("ANONYMOUS message (optional):", None::<&str>)?;
        let message = Some(message).filter(|message| !message.trim().is_empty());

        return Ok(SaslConfig::Anonymous(SaslAnonymousConfig { message }));
    }

    let login = prompt::text("Login:", login_hint)?;
    let key = format!("{account_name}-imap");

    Ok(match mechanism {
        SaslMechanism::Plain => SaslConfig::Plain(SaslPlainConfig {
            authzid: None,
            authcid: login,
            passwd: secret::configure_password("IMAP password", &key)?,
        }),
        SaslMechanism::Login => SaslConfig::Login(SaslLoginConfig {
            username: login,
            password: secret::configure_password("IMAP password", &key)?,
        }),
        SaslMechanism::ScramSha256 => SaslConfig::ScramSha256(SaslScramSha256Config {
            username: login,
            password: secret::configure_password("IMAP password", &key)?,
        }),
        SaslMechanism::OAuthBearer => SaslConfig::Oauthbearer(SaslOauthbearerConfig {
            username: login,
            token: secret::configure_token("IMAP API token", &key, caps.oauth || !caps.any())?,
        }),
        SaslMechanism::XOAuth2 => SaslConfig::Xoauth2(SaslXoauth2Config {
            username: login,
            token: secret::configure_token("IMAP API token", &key, caps.oauth || !caps.any())?,
        }),
        SaslMechanism::Anonymous => unreachable!("handled above"),
        // NOTE: io-sasl knows more mechanisms than the config can express,
        // and the menu only offers the six above, so this arm bails rather
        // than panics in case a caller hands one over directly.
        other => bail!("Unsupported SASL mechanism `{}`", other.as_str()),
    })
}

/// Whether the configuration can express a mechanism, which keeps the
/// menu to what `imap.sasl` has a table for.
///
/// A server advertising one carillon cannot write down is one mechanism
/// fewer to choose from, not a prompt ending in an error.
fn is_expressible(mechanism: &SaslMechanism) -> bool {
    matches!(
        mechanism,
        SaslMechanism::ScramSha256
            | SaslMechanism::Plain
            | SaslMechanism::OAuthBearer
            | SaslMechanism::XOAuth2
            | SaslMechanism::Anonymous
            | SaslMechanism::Login
    )
}

/// The mechanisms the menu offers: what the probe read, or what discovery
/// keys when it read nothing.
fn offered(caps: AuthCaps, probed: Option<&[SaslMechanism]>) -> Vec<SaslMechanism> {
    match probed {
        Some(mechanisms) if !mechanisms.is_empty() => mechanisms.to_vec(),
        _ => discovered_mechanisms(caps),
    }
}

/// The mechanisms offered when no probe answered, keyed on what discovery
/// advertised, every family when nothing was.
///
/// A coarse guess: a provider's policy says a password is accepted, never
/// which mechanism carries it. Only the server knows that, which is why
/// this list is the fallback rather than the menu.
fn discovered_mechanisms(caps: AuthCaps) -> Vec<SaslMechanism> {
    let mut mechanisms = Vec::new();

    if caps.basic || !caps.any() {
        mechanisms.extend([SaslMechanism::ScramSha256, SaslMechanism::Plain]);
    }

    if caps.token() || !caps.any() {
        mechanisms.extend([SaslMechanism::OAuthBearer, SaslMechanism::XOAuth2]);
    }

    if caps.basic || !caps.any() {
        mechanisms.extend([SaslMechanism::Anonymous, SaslMechanism::Login]);
    }

    mechanisms
}

/// The menu label of a mechanism, split by the credential it needs.
///
/// Only the six the menu offers get one, anything else being named as
/// IANA registered it.
fn mechanism_label(mechanism: &SaslMechanism) -> &'static str {
    match mechanism {
        SaslMechanism::ScramSha256 => SCRAM_SHA_256,
        SaslMechanism::Plain => PLAIN,
        SaslMechanism::OAuthBearer => OAUTHBEARER,
        SaslMechanism::XOAuth2 => XOAUTH2,
        SaslMechanism::Anonymous => ANONYMOUS,
        SaslMechanism::Login => LOGIN,
        other => other.as_str(),
    }
}

/// Folds the endpoint and credentials into a block watching the inbox,
/// the mailbox a first account watches and the one every server has.
fn config(endpoint: &TcpEndpoint, sasl: Option<SaslConfig>) -> ImapConfig {
    let scheme = if endpoint.security == DiscoverySecurity::Tls {
        "imaps"
    } else {
        "imap"
    };

    ImapConfig {
        mailbox: String::from("INBOX"),
        server: format!("{scheme}://{}:{}", endpoint.host, endpoint.port),
        tls: Default::default(),
        alpn: None,
        starttls: endpoint.security == DiscoverySecurity::Starttls,
        sasl,
        sasl_ir: None,
        id: Default::default(),
        watch: None,
        hook: hook(),
    }
}

/// The hook a generated account fires: a notification on arrival, which
/// is what someone watching a mailbox came for.
///
/// IMAP resolves an arrival's envelope, so the notification may name it.
fn hook() -> ImapHookConfig {
    ImapHookConfig {
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
    fn the_server_decides_the_menu_and_discovery_only_stands_in() {
        // NOTE: the Gmail shape, a provider whose policy names a password
        // and a grant on a server that never implemented SCRAM. What it
        // advertises is the menu, so SCRAM is not in it.
        let gmail = AuthCaps {
            basic: true,
            oauth: true,
            ..Default::default()
        };
        let advertised = [
            SaslMechanism::Plain,
            SaslMechanism::OAuthBearer,
            SaslMechanism::XOAuth2,
            SaslMechanism::Login,
        ];

        assert_eq!(offered(gmail, Some(&advertised)), advertised);
        assert!(discovered_mechanisms(gmail).contains(&SaslMechanism::ScramSha256));

        // NOTE: a probe that read nothing, or nothing usable, falls back
        // to the discovered list rather than leaving the menu empty.
        assert_eq!(offered(gmail, None), discovered_mechanisms(gmail));
        assert_eq!(offered(gmail, Some(&[])), discovered_mechanisms(gmail));
    }

    #[test]
    fn a_mechanism_the_config_cannot_write_down_is_not_offered() {
        assert!(is_expressible(&SaslMechanism::ScramSha256));
        assert!(is_expressible(&SaslMechanism::XOAuth2));
        assert!(!is_expressible(&SaslMechanism::ScramSha512));
        assert!(!is_expressible(&SaslMechanism::CramMd5));
    }

    #[test]
    fn advertised_methods_narrow_the_mechanism_menu() {
        let basic = AuthCaps {
            basic: true,
            ..Default::default()
        };
        assert_eq!(
            discovered_mechanisms(basic),
            [
                SaslMechanism::ScramSha256,
                SaslMechanism::Plain,
                SaslMechanism::Anonymous,
                SaslMechanism::Login,
            ]
        );

        let token = AuthCaps {
            bearer: true,
            oauth: true,
            ..Default::default()
        };
        assert_eq!(
            discovered_mechanisms(token),
            [SaslMechanism::OAuthBearer, SaslMechanism::XOAuth2]
        );

        // NOTE: nothing advertised offers everything, so a service naming
        // no method is still configurable.
        assert_eq!(discovered_mechanisms(AuthCaps::default()).len(), 6);
    }

    #[test]
    fn a_generated_account_watches_the_inbox_and_notifies_on_arrival() {
        let endpoint = TcpEndpoint {
            host: String::from("imap.example.org"),
            port: 993,
            security: DiscoverySecurity::Tls,
        };
        let config = config(
            &endpoint,
            Some(SaslConfig::Anonymous(SaslAnonymousConfig::default())),
        );

        assert_eq!(config.mailbox, "INBOX");
        assert_eq!(config.server, "imaps://imap.example.org:993");
        assert!(!config.starttls);
        assert!(config.watch.is_none());
        assert!(config.hook.on_message_added.is_some());
    }

    #[test]
    fn a_starttls_endpoint_keeps_the_cleartext_scheme() {
        let endpoint = TcpEndpoint {
            host: String::from("imap.example.org"),
            port: 143,
            security: DiscoverySecurity::Starttls,
        };
        let config = config(
            &endpoint,
            Some(SaslConfig::Anonymous(SaslAnonymousConfig::default())),
        );

        assert_eq!(config.server, "imap://imap.example.org:143");
        assert!(config.starttls);
    }
}
