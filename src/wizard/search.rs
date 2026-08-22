//! Service discovery for the wizard, from one email address.
//!
//! The typed address feeds io-pim-discovery's parallel discovery (fixed
//! provider rules, PACC, Mozilla autoconfig, RFC 6186 SRV, RFC 6764
//! DAV resolve, RFC 8620 JMAP resolve), and every reachable service
//! carillon can watch becomes one selectable entry carrying the
//! authentication capabilities it advertised. The concrete method is
//! picked once the service is chosen, so a service appears exactly once
//! in the list.

use std::{collections::BTreeSet, env, fmt, time::Duration};

use anyhow::Result;
use io_pim_discovery::{
    compose::{
        client::DiscoveryComposeClientStd,
        config::{
            DiscoveryAuthMethod, DiscoveryEndpoint, DiscoverySecurity, DiscoveryService,
            DiscoveryServiceConfig,
        },
    },
    shared::dns::system_resolver,
};
use pimalaya_stream::tls::{Rustls, Tls};
use url::Url;

/// DNS-over-TCP resolver backing discovery when `CARILLON_DNS_RESOLVER`
/// is unset and no system resolver is found: Cloudflare's `1.1.1.1`.
const DEFAULT_RESOLVER: &str = "tcp://1.1.1.1:53";

/// Upper bound on the parallel discovery fan-out. An unreachable
/// endpoint (a firewalled port, a black-hole host) must not stall the
/// interactive wizard, so mechanisms that have not reported by then are
/// abandoned and only what completed in time is offered.
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(8);

/// One selectable service to watch, carrying the authentication
/// capabilities it advertised.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Discovered {
    pub kind: DiscoveredKind,
    /// Login hint advertised by the mechanism (usually the email).
    pub username: Option<String>,
    /// What the service accepts, folded across its discovered methods.
    pub auth: AuthCaps,
}

/// The discovered service kind, carrying the endpoint to watch.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiscoveredKind {
    /// An IMAP endpoint, watched over IDLE.
    Imap(TcpEndpoint),
    /// A JMAP session endpoint, watched over its event stream.
    Jmap(String),
    /// A CalDAV context root, whose calendars are listed once the
    /// credential is known.
    Caldav(String),
    /// A CardDAV context root, whose addressbooks are listed the same
    /// way.
    Carddav(String),
}

/// A discovered TCP service endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TcpEndpoint {
    pub host: String,
    pub port: u16,
    pub security: DiscoverySecurity,
}

/// The authentication capabilities a service advertised, folded across
/// all its discovered methods. It decides what the per-service auth
/// prompt offers: which SASL mechanisms or HTTP schemes, and whether
/// the OAuth token brokers appear. carillon reads a token an external
/// manager (such as Ortie) issues but never runs a grant itself, so
/// OAuth is not a method of its own here: it only unlocks the brokers
/// behind the API token flow (see [`super::secret`]).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuthCaps {
    /// Basic/password auth: SASL PLAIN/LOGIN/SCRAM for IMAP, Basic for
    /// the HTTP services. Often an app password (e.g. Fastmail).
    pub basic: bool,
    /// A static bearer/API token: SASL OAUTHBEARER/XOAUTH2 for IMAP,
    /// Bearer for the HTTP services.
    pub bearer: bool,
    /// An OAuth 2.0 grant is advertised, so a broker can issue the
    /// token.
    pub oauth: bool,
}

impl AuthCaps {
    /// Whether any capability was advertised. When none was (a
    /// mechanism that names no auth), the auth prompt offers every
    /// method so the user is never left without a choice.
    pub fn any(self) -> bool {
        self.basic || self.bearer || self.oauth
    }

    /// Whether a token (static or broker-issued) is on offer.
    pub fn token(self) -> bool {
        self.bearer || self.oauth
    }
}

impl fmt::Display for Discovered {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            DiscoveredKind::Imap(imap) => write!(f, "IMAP {}", imap.host),
            DiscoveredKind::Jmap(url) => write!(f, "JMAP {url}"),
            DiscoveredKind::Caldav(url) => write!(f, "CalDAV {url}"),
            DiscoveredKind::Carddav(url) => write!(f, "CardDAV {url}"),
        }
    }
}

impl Discovered {
    /// Best default login for the credential prompt: the advertised
    /// username when it looks like an address, else the searched email
    /// when the user typed a full one, else nothing (a bare domain,
    /// whose synthesized `@domain` form is rejected here).
    pub fn login_default(&self, email: &str) -> Option<String> {
        self.username
            .clone()
            .filter(|username| looks_like_address(username))
            .or_else(|| looks_like_address(email).then(|| email.to_string()))
    }

    /// Ranks an entry for the selection list, in the order the backend
    /// selector already picks a configured block: IMAP, JMAP, CalDAV,
    /// CardDAV.
    fn rank(&self) -> u8 {
        match self.kind {
            DiscoveredKind::Imap(_) => 0,
            DiscoveredKind::Jmap(_) => 1,
            DiscoveredKind::Caldav(_) => 2,
            DiscoveredKind::Carddav(_) => 3,
        }
    }
}

/// Searches every service reachable from `email` that carillon can
/// watch, and returns one selectable entry per service, ordered by
/// [`Discovered::rank`].
pub fn search(email: &str) -> Result<Vec<Discovered>> {
    let client = DiscoveryComposeClientStd::new(discovery_resolver(), discovery_tls());
    let services = BTreeSet::from([
        DiscoveryService::Imap,
        DiscoveryService::Jmap,
        DiscoveryService::Caldav,
        DiscoveryService::Carddav,
    ]);
    let configs = client.compose_all_within(email, services, DISCOVERY_TIMEOUT)?;

    let mut found = Vec::new();

    if let Some(imap) = best_tcp(&configs, DiscoveryService::Imap) {
        found.push(Discovered {
            kind: DiscoveredKind::Imap(imap.0),
            username: imap.1.username.clone(),
            auth: caps_of(&imap.1.auth),
        });
    }

    // NOTE: the HTTP services are keyed on their endpoint, since the
    // mechanisms overlap: SRV and PACC routinely name the same root,
    // and offering it twice is a choice with no difference.
    for (service, kind) in [
        (
            DiscoveryService::Jmap,
            DiscoveredKind::Jmap as fn(String) -> DiscoveredKind,
        ),
        (DiscoveryService::Caldav, DiscoveredKind::Caldav),
        (DiscoveryService::Carddav, DiscoveredKind::Carddav),
    ] {
        let mut urls: Vec<(String, Option<String>, AuthCaps)> = Vec::new();

        for config in configs.iter().filter(|c| c.service == service) {
            let DiscoveryEndpoint::Http(url) = &config.endpoint else {
                continue;
            };

            let caps = caps_of(&config.auth);

            match urls.iter_mut().find(|(known, _, _)| known == url) {
                Some((_, _, known)) => {
                    known.basic |= caps.basic;
                    known.bearer |= caps.bearer;
                    known.oauth |= caps.oauth;
                }
                None => urls.push((url.clone(), config.username.clone(), caps)),
            }
        }

        found.extend(urls.into_iter().map(|(url, username, auth)| Discovered {
            kind: kind(url),
            username,
            auth,
        }));
    }

    found.sort_by_key(Discovered::rank);

    Ok(found)
}

/// Folds a service's advertised methods into its [`AuthCaps`]: password
/// into `basic`, bearer into `bearer`, and every OAuth grant into
/// `oauth` (which only unlocks the token brokers, never a self-run
/// grant).
fn caps_of(auth: &[DiscoveryAuthMethod]) -> AuthCaps {
    let mut caps = AuthCaps::default();

    for method in auth {
        match method {
            DiscoveryAuthMethod::Password => caps.basic = true,
            DiscoveryAuthMethod::Bearer => caps.bearer = true,
            _ => caps.oauth = true,
        }
    }

    caps
}

/// Picks the best endpoint for a TCP service: the most secure one wins,
/// so a domain advertising both implicit TLS and STARTTLS keeps the
/// former.
fn best_tcp(
    configs: &[DiscoveryServiceConfig],
    service: DiscoveryService,
) -> Option<(TcpEndpoint, &DiscoveryServiceConfig)> {
    let config = configs
        .iter()
        .filter(|config| config.service == service)
        .max_by_key(|config| match &config.endpoint {
            DiscoveryEndpoint::Tcp {
                security: DiscoverySecurity::Tls,
                ..
            } => 2,
            DiscoveryEndpoint::Tcp {
                security: DiscoverySecurity::Starttls,
                ..
            } => 1,
            _ => 0,
        })?;

    let DiscoveryEndpoint::Tcp {
        host,
        port,
        security,
    } = &config.endpoint
    else {
        return None;
    };

    Some((
        TcpEndpoint {
            host: host.clone(),
            port: *port,
            security: *security,
        },
        config,
    ))
}

/// Whether a string is a full `local@domain` address (both parts
/// non-empty), rejecting the bare-domain `@domain` form.
fn looks_like_address(value: &str) -> bool {
    value
        .split_once('@')
        .is_some_and(|(local, domain)| !local.is_empty() && !domain.is_empty())
}

/// Resolver used by discovery: the `CARILLON_DNS_RESOLVER` override
/// first, then the system resolver (`/etc/resolv.conf` on unix, the
/// network adapters on windows), then the Cloudflare default. This
/// avoids leaking the email domain to a third-party resolver and works
/// around networks that block the default.
fn discovery_resolver() -> Url {
    if let Ok(resolver) = env::var("CARILLON_DNS_RESOLVER")
        && let Ok(url) = resolver.parse()
    {
        return url;
    }

    if let Some(url) = system_resolver() {
        return url;
    }

    DEFAULT_RESOLVER
        .parse()
        .expect("DEFAULT_RESOLVER must be a valid URL")
}

/// TLS profile for the HTTPS-bound discovery mechanisms; they only
/// speak HTTP/1.1 to `_well-known` endpoints.
fn discovery_tls() -> Tls {
    Tls {
        rustls: Rustls {
            alpn: vec!["http/1.1".into()],
            ..Default::default()
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_fold_each_method_onto_its_axis() {
        let oauth = DiscoveryAuthMethod::OauthIssuer("https://issuer".into());

        assert_eq!(
            caps_of(&[DiscoveryAuthMethod::Password]),
            AuthCaps {
                basic: true,
                ..Default::default()
            }
        );
        assert_eq!(
            caps_of(&[DiscoveryAuthMethod::Bearer]),
            AuthCaps {
                bearer: true,
                ..Default::default()
            }
        );

        // NOTE: the Fastmail shape, bearer plus an OAuth grant and no
        // Basic, is one "API token" method whose brokers are unlocked.
        let fastmail = caps_of(&[DiscoveryAuthMethod::Bearer, oauth]);
        assert!(fastmail.token());
        assert!(!fastmail.basic);
        assert!(fastmail.any());
    }

    #[test]
    fn caps_report_emptiness_and_token_offer() {
        assert!(!AuthCaps::default().any());
        assert!(!AuthCaps::default().token());
    }

    #[test]
    fn the_login_default_rejects_the_synthesized_bare_domain() {
        let entry = Discovered {
            kind: DiscoveredKind::Jmap("https://api.example.org/jmap/session".into()),
            username: None,
            auth: AuthCaps::default(),
        };

        assert_eq!(
            entry.login_default("alice@example.org").as_deref(),
            Some("alice@example.org")
        );
        assert_eq!(entry.login_default("@example.org"), None);
    }

    #[test]
    fn an_advertised_username_wins_over_the_searched_address() {
        let entry = Discovered {
            kind: DiscoveredKind::Caldav("https://dav.example.org/".into()),
            username: Some("alice.doe@example.org".into()),
            auth: AuthCaps::default(),
        };

        assert_eq!(
            entry.login_default("alice@example.org").as_deref(),
            Some("alice.doe@example.org")
        );
    }

    #[test]
    fn the_selection_list_follows_the_backend_priority() {
        let entry = |kind| Discovered {
            kind,
            username: None,
            auth: AuthCaps::default(),
        };
        let mut found = [
            entry(DiscoveredKind::Carddav("https://dav".into())),
            entry(DiscoveredKind::Jmap("https://jmap".into())),
            entry(DiscoveredKind::Imap(TcpEndpoint {
                host: "imap.example.org".into(),
                port: 993,
                security: DiscoverySecurity::Tls,
            })),
        ];
        found.sort_by_key(Discovered::rank);

        let names: Vec<String> = found.iter().map(ToString::to_string).collect();
        assert_eq!(
            names,
            [
                "IMAP imap.example.org",
                "JMAP https://jmap",
                "CardDAV https://dav"
            ]
        );
    }
}
