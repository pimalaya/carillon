//! Account discovery, the half of the wizard that decides what the
//! account watches.
//!
//! What becomes of the discovered account, a file to create, a block to
//! append or a document on stdout, belongs to [`super::configure`],
//! which is also where the welcome and the prompts around this one
//! live.
//!
//! One prompt takes an email address, a server URL, or a local folder
//! path, and its shape orients the setup:
//!
//! - an email (or bare domain) searches every service carillon can
//!   watch (see [`super::search`]) and every reachable one becomes a
//!   selectable entry; picking one then prompts its authentication
//!   method among those advertised;
//! - a `scheme://` URL discovers from its host, its scheme narrowing
//!   the results;
//! - an existing folder is a local Maildir.
//!
//! The wizard only configures what it can discover automatically. When
//! discovery finds nothing for the given input it stops and points at
//! the documented sample, rather than prompting for a hand-entered
//! config.
//!
//! carillon runs no OAuth 2.0 grant itself: a grant only unlocks the
//! external token brokers (Ortie, pizauth, oama) behind the API token
//! credential prompt (see [`super::secret`]).

use std::path::PathBuf;

#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
use anyhow::Context;
use anyhow::{Result, bail};
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
use io_pim_discovery::compose::config::DiscoverySecurity;
use pimalaya_cli::prompt;
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
use pimalaya_cli::spinner::Spinner;
use url::Url;

use crate::config::AccountConfig;
#[cfg(feature = "dav")]
use crate::wizard::dav;
#[cfg(feature = "imap")]
use crate::wizard::imap;
#[cfg(feature = "jmap")]
use crate::wizard::jmap;
#[cfg(feature = "maildir")]
use crate::wizard::local;
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
use crate::{
    config::CONFIG_SAMPLE_URL,
    wizard::search::{self, Discovered, DiscoveredKind},
};

/// Discovers one account from a single prompt, tests it, and hands
/// back the name it proposes with the account itself.
///
/// Every flow tests the connection it configured, since that
/// connection is also what tells the wizard the collection to watch
/// and the best method to watch it with. What happens to the account
/// afterwards belongs to [`super::configure`].
pub fn run() -> Result<(String, AccountConfig)> {
    let input = prompt::text("Email:", None)?;
    let input = input.trim();

    if input.is_empty() {
        bail!("Empty input: enter an email address, a server URL, or a folder path");
    }

    // NOTE: the account name is just the TOML table key, so it is
    // derived from the input rather than prompted; the user renames it
    // by hand.
    let account_name = default_account_name(input);
    let account = build_account(&account_name, input)?;

    Ok((account_name, account))
}

/// Orients the setup from the input shape, then folds the configured
/// backend into a fresh [`AccountConfig`].
///
/// The account is left non-default here. Whether it claims the default
/// depends on what the configuration already holds, which discovery
/// does not read, so [`super::configure`] decides it.
fn build_account(account_name: &str, input: &str) -> Result<AccountConfig> {
    let account = AccountConfig::default();

    if is_path(input) {
        return configure_local(account, input);
    }

    configure_discovered(account_name, input, account)
}

/// Searches the services reachable from the input, lets one be picked,
/// and configures it.
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
fn configure_discovered(
    account_name: &str,
    input: &str,
    mut account: AccountConfig,
) -> Result<AccountConfig> {
    // A `scheme://host` URL discovers from its host, and its scheme
    // narrows the results; an email or bare domain discovers from the
    // domain with no scheme filter.
    let (email, scheme) = if input.contains("://") {
        let url = Url::parse(input).with_context(|| format!("Invalid server URL `{input}`"))?;
        let host = url.host_str().unwrap_or_default().to_string();
        (format!("@{host}"), Some(url.scheme().to_string()))
    } else if input.contains('@') {
        (input.to_string(), None)
    } else {
        (format!("@{input}"), None)
    };

    let spinner = Spinner::start("Searching for server settings");
    let mut found = search::search(&email)?;
    retain_supported(&mut found);

    if let Some(scheme) = &scheme {
        retain_scheme(&mut found, scheme)?;
    }

    if found.is_empty() {
        spinner.failure("No configuration found");
        return stop_undiscovered(input);
    }

    spinner.success(format!("Found {} configuration(s)", found.len()));

    let default = found.first().cloned();
    let choice = prompt::item("Choose a configuration:", found, default)?;

    dispatch(account_name, &email, choice, &mut account)?;

    Ok(account)
}

#[cfg(not(any(feature = "imap", feature = "jmap", feature = "dav")))]
fn configure_discovered(
    _account_name: &str,
    input: &str,
    _account: AccountConfig,
) -> Result<AccountConfig> {
    bail!("`{input}` names a server, but no network backend is compiled in")
}

/// Configures the backend behind a discovered entry, on the account
/// that will carry it.
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
#[cfg_attr(
    all(feature = "imap", feature = "jmap", feature = "dav"),
    allow(unreachable_patterns)
)]
fn dispatch(
    account_name: &str,
    email: &str,
    choice: Discovered,
    account: &mut AccountConfig,
) -> Result<()> {
    match &choice.kind {
        #[cfg(feature = "imap")]
        DiscoveredKind::Imap(_) => {
            account.imap = Some(imap::configure_discovered(account_name, email, &choice)?);
            Ok(())
        }
        #[cfg(feature = "jmap")]
        DiscoveredKind::Jmap(_) => {
            account.jmap = Some(jmap::configure_discovered(account_name, email, &choice)?);
            Ok(())
        }
        #[cfg(feature = "dav")]
        DiscoveredKind::Caldav(_) | DiscoveredKind::Carddav(_) => {
            match dav::configure_discovered(account_name, email, &choice)? {
                dav::Dav::Caldav(config) => account.caldav = Some(*config),
                dav::Dav::Carddav(config) => account.carddav = Some(*config),
            }

            Ok(())
        }
        kind => bail!("Configuration `{kind:?}` is not supported by this build"),
    }
}

/// Configures a local backend from a typed folder path.
#[cfg(feature = "maildir")]
fn configure_local(mut account: AccountConfig, input: &str) -> Result<AccountConfig> {
    account.maildir = Some(local::configure(root(input))?);

    Ok(account)
}

#[cfg(not(feature = "maildir"))]
fn configure_local(_account: AccountConfig, input: &str) -> Result<AccountConfig> {
    bail!("`{input}` looks like a folder path, but no local backend is compiled in")
}

/// Keeps only the discovered entries a `scheme://` URL asked for:
/// `imap` and `imaps` keep IMAP (with `imaps` requiring an
/// implicit-TLS endpoint), the HTTP-family schemes keep every service
/// that speaks over HTTP, since a DAV root and a JMAP session are told
/// apart by the path rather than by the scheme. An unknown scheme is
/// rejected outright.
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
fn retain_scheme(found: &mut Vec<Discovered>, scheme: &str) -> Result<()> {
    match scheme {
        "imap" | "imaps" => {
            let tls_only = scheme == "imaps";
            found.retain(|entry| match &entry.kind {
                DiscoveredKind::Imap(imap) => !tls_only || imap.security == DiscoverySecurity::Tls,
                _ => false,
            });
        }
        "jmap" | "jmaps" | "http" | "https" | "caldav" | "caldavs" | "carddav" | "carddavs" => {
            found.retain(|entry| !matches!(entry.kind, DiscoveredKind::Imap(_)));
        }
        other => bail!("Unsupported server scheme `{other}`"),
    }

    Ok(())
}

/// Stops the wizard when discovery found nothing to configure for
/// `input`: it prints where to go next (a hand-written config, seeded
/// from the documented sample) and errors out, rather than dropping
/// into a hand-entry flow. carillon's wizard only ever configures what
/// it can discover automatically.
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
fn stop_undiscovered(input: &str) -> Result<AccountConfig> {
    bail!(
        "Could not automatically discover a configuration for `{input}`.\n\n\
         Write your account configuration by hand instead, starting from the \
         documented sample:\n  {CONFIG_SAMPLE_URL}"
    )
}

/// Drops the discovered entries whose backend is not compiled in.
#[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
fn retain_supported(found: &mut Vec<Discovered>) {
    found.retain(|entry| match entry.kind {
        DiscoveredKind::Imap(_) => cfg!(feature = "imap"),
        DiscoveredKind::Jmap(_) => cfg!(feature = "jmap"),
        DiscoveredKind::Caldav(_) | DiscoveredKind::Carddav(_) => cfg!(feature = "dav"),
    });
}

/// Proposes a default account name from the input shape: the first
/// label of the domain (of an email, host, or bare domain), or the
/// folder name of a local path.
fn default_account_name(input: &str) -> String {
    if is_path(input) {
        return root(input)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("personal")
            .to_string();
    }

    if let Ok(url) = Url::parse(input)
        && let Some(host) = url.host_str()
    {
        return first_label(host);
    }

    match input.rsplit_once('@') {
        Some((_, domain)) => first_label(domain),
        None => first_label(input),
    }
}

/// The first dot-separated label of a host or domain.
fn first_label(host: &str) -> String {
    host.split('.').next().unwrap_or(host).to_string()
}

/// The path an input names, with `file://` stripped and `~` expanded.
fn root(input: &str) -> PathBuf {
    let raw = input.strip_prefix("file://").unwrap_or(input);

    PathBuf::from(shellexpand::tilde(raw).into_owned())
}

/// Whether the input names a filesystem path (absolute, home-relative,
/// explicitly relative, or a `file://` URL) rather than a network
/// endpoint.
fn is_path(input: &str) -> bool {
    input.starts_with("file://")
        || input.starts_with('/')
        || input.starts_with('~')
        || input.starts_with("./")
        || input.starts_with("../")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_name_defaults_to_the_first_domain_label() {
        // Email: the domain's first label, never the local part.
        assert_eq!(default_account_name("clement.douin@posteo.net"), "posteo");
        assert_eq!(default_account_name("alice@mail.example.co.uk"), "mail");
        // Bare domain (as discovery synthesizes it) and plain domain.
        assert_eq!(default_account_name("@posteo.net"), "posteo");
        assert_eq!(default_account_name("posteo.net"), "posteo");
    }

    #[test]
    fn account_name_defaults_to_the_last_path_component() {
        assert_eq!(
            default_account_name("/home/alice/mail/personal"),
            "personal"
        );
        assert_eq!(default_account_name("file:///var/mail/archive"), "archive");
    }

    #[test]
    fn a_folder_is_told_apart_from_an_endpoint() {
        assert!(is_path("/var/mail/alice"));
        assert!(is_path("~/Mail/personal"));
        assert!(is_path("./mail"));
        assert!(is_path("file:///var/mail/alice"));

        assert!(!is_path("alice@example.org"));
        assert!(!is_path("example.org"));
        assert!(!is_path("imaps://imap.example.org"));
    }

    #[cfg(any(feature = "imap", feature = "jmap", feature = "dav"))]
    #[test]
    fn a_scheme_keeps_only_what_it_named() {
        let entry = |kind| Discovered {
            kind,
            username: None,
            auth: Default::default(),
        };
        let imap = |security| {
            entry(DiscoveredKind::Imap(search::TcpEndpoint {
                host: String::from("imap.example.org"),
                port: 143,
                security,
            }))
        };
        let all = || {
            vec![
                imap(DiscoverySecurity::Starttls),
                entry(DiscoveredKind::Jmap(String::from("https://jmap"))),
                entry(DiscoveredKind::Caldav(String::from("https://dav"))),
            ]
        };

        let mut found = all();
        retain_scheme(&mut found, "imap").expect("retain imap");
        assert_eq!(found.len(), 1);

        // `imaps` asks for implicit TLS, which the STARTTLS endpoint is
        // not.
        let mut found = all();
        retain_scheme(&mut found, "imaps").expect("retain imaps");
        assert!(found.is_empty());

        let mut found = all();
        retain_scheme(&mut found, "https").expect("retain https");
        assert_eq!(found.len(), 2);

        assert!(retain_scheme(&mut all(), "ftp").is_err());
    }
}
