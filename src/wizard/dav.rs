//! # DAV wizard
//!
//! Discovery names a context root, which is not yet something to watch:
//! an account watches one collection, and which collections exist is
//! behind the credential.
//!
//! So [`configure_discovered`] prompts the HTTP authentication scheme,
//! connects, walks the principal and the home-set, and offers the
//! calendars or the addressbooks it holds. That walk is the connection
//! test.
//!
//! The connection is opened here rather than through [`crate::dav`]: the
//! watch arms a read deadline so a poll cannot outlive a Ctrl+C, and a
//! listing has no such deadline to respect.

use anyhow::{Context, Result, bail};
use io_http::client::HttpClientStd;
use io_webdav::client::WebdavClientStd;
use pimalaya_cli::{prompt, spinner::Spinner};
use pimalaya_config::secret::SecretResolver;
use url::Url;

use crate::{
    config::{
        CaldavConfig, CaldavHookConfig, CarddavConfig, CarddavHookConfig, DavAuthConfig, ItemHook,
        NotifyConfig, TlsConfig,
    },
    dav,
    wizard::{
        search::{AuthCaps, Discovered, DiscoveredKind},
        secret,
    },
};

const BASIC: &str = "Basic (username + password)";
const BEARER: &str = "Bearer (API token)";

/// The iCalendar components a calendar may hold, as RFC 4791 §5.2.3 names
/// them in `supported-calendar-component-set`.
const VEVENT: &str = "VEVENT";
const VTODO: &str = "VTODO";

/// The DAV backend a discovered entry configured.
pub enum Dav {
    /// A calendar to watch.
    Caldav(Box<CaldavConfig>),
    /// An addressbook to watch.
    Carddav(Box<CarddavConfig>),
}

/// Configures CalDAV or CardDAV from a discovered entry, whose context
/// root is pinned.
///
/// The scheme is picked among those advertised, skipped when only one
/// qualifies, then the collection to watch among those the account holds.
pub fn configure_discovered(
    account_name: &str,
    email: &str,
    discovered: &Discovered,
) -> Result<Dav> {
    let (server, calendars) = match &discovered.kind {
        DiscoveredKind::Caldav(server) => (server, true),
        DiscoveredKind::Carddav(server) => (server, false),
        _ => bail!("Expected a CalDAV or CardDAV configuration"),
    };

    let server =
        Url::parse(server).with_context(|| format!("Invalid DAV server URL `{server}`"))?;
    let label = if calendars { "CalDAV" } else { "CardDAV" };
    let auth = prompt_auth(
        label,
        account_name,
        discovered.login_default(email).as_deref(),
        discovered.auth,
    )?;

    let mut client = connect(label, &server, &auth)?;

    if calendars {
        let (home, collection) = prompt_calendar(&mut client)?;

        Ok(Dav::Caldav(Box::new(CaldavConfig {
            calendar: collection.path,
            server: origin(home),
            tls: Default::default(),
            alpn: None,
            auth,
            watch: None,
            hook: caldav_hook(&collection.components),
        })))
    } else {
        let (home, collection) = prompt_addressbook(&mut client)?;

        Ok(Dav::Carddav(Box::new(CarddavConfig {
            addressbook: collection.path,
            server: origin(home),
            tls: Default::default(),
            alpn: None,
            auth,
            watch: None,
            hook: carddav_hook(),
        })))
    }
}

/// Opens the connection the listing runs on, which is also the test.
///
/// A bad credential or an unreachable host fails here rather than
/// yielding an account that cannot connect.
fn connect(label: &str, server: &Url, auth: &DavAuthConfig) -> Result<WebdavClientStd> {
    let spinner = Spinner::start(format!("Testing {label} connection"));

    // NOTE: the same profile a watch opens with, an account carrying no
    // `alpn` key yet when the wizard tests it.
    let tls = TlsConfig::default().into_tls(HttpClientStd::default_alpn());

    let opened = dav::auth(auth, &mut SecretResolver::new())
        .and_then(|auth| Ok(WebdavClientStd::connect(server, &tls, auth)?));

    match opened {
        Ok(client) => {
            spinner.success(format!("{label} connection succeeded"));
            Ok(client)
        }
        Err(err) => {
            spinner.failure(format!("{label} connection failed"));
            Err(err)
        }
    }
}

/// Prompts the HTTP authentication scheme from `caps`, both offered when
/// none was advertised, then its credentials.
///
/// The Bearer flow shows the OAuth brokers only when a grant was
/// advertised.
fn prompt_auth(
    label: &str,
    account_name: &str,
    login_hint: Option<&str>,
    caps: AuthCaps,
) -> Result<DavAuthConfig> {
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
        prompt::item(format!("{label} authentication:"), schemes, None)?
    };

    let key = format!("{account_name}-{}", label.to_lowercase());

    Ok(match scheme {
        BASIC => DavAuthConfig::Basic {
            username: prompt::text(format!("{label} username:"), login_hint.map(str::to_string))?,
            password: secret::configure_password(&format!("{label} password"), &key)?,
        },
        _ => DavAuthConfig::Bearer {
            token: secret::configure_token(
                &format!("{label} API token"),
                &key,
                caps.oauth || !caps.any(),
            )?,
        },
    })
}

/// Lists the calendars of the account and asks which one to watch, with
/// the home-set they hang under.
fn prompt_calendar(client: &mut WebdavClientStd) -> Result<(Url, Choice)> {
    let home = client.calendar_home_set()?;
    let calendars = client.list_calendars()?;

    if calendars.is_empty() {
        bail!("No calendar found under {home}");
    }

    let choices: Vec<Choice> = calendars
        .into_iter()
        .map(|calendar| Choice {
            label: label(calendar.display_name.as_deref(), &calendar.id),
            path: path(&home, &calendar.id),
            components: calendar.components.into_iter().collect(),
        })
        .collect();

    let choice = pick("Calendar to watch:", choices)?;

    Ok((home, choice))
}

/// Lists the addressbooks of the account and asks which one to watch,
/// with the home-set they hang under.
fn prompt_addressbook(client: &mut WebdavClientStd) -> Result<(Url, Choice)> {
    let home = client.addressbook_home_set()?;
    let addressbooks = client.list_addressbooks()?;

    if addressbooks.is_empty() {
        bail!("No addressbook found under {home}");
    }

    let choices: Vec<Choice> = addressbooks
        .into_iter()
        .map(|addressbook| Choice {
            label: label(addressbook.display_name.as_deref(), &addressbook.id),
            path: path(&home, &addressbook.id),
            components: Vec::new(),
        })
        .collect();

    let choice = pick("Addressbook to watch:", choices)?;

    Ok((home, choice))
}

/// One listed collection, as the pick list shows it and the account
/// records it.
struct Choice {
    label: String,
    path: String,
    components: Vec<String>,
}

impl core::fmt::Display for Choice {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.label)
    }
}

impl PartialEq for Choice {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for Choice {}

/// Asks which collection to watch, selecting the only one there is
/// without prompting.
fn pick(prompt: &str, mut choices: Vec<Choice>) -> Result<Choice> {
    if choices.len() == 1 {
        return Ok(choices.remove(0));
    }

    Ok(prompt::item(prompt, choices, None)?)
}

/// The pick-list label of a collection: its display name, falling back to
/// the last segment of its URL, which the account records.
fn label(display_name: Option<&str>, id: &str) -> String {
    match display_name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => format!("{name} ({id})"),
        None => id.to_string(),
    }
}

/// The absolute path of a collection under its home-set, which is what
/// `caldav.calendar` and `carddav.addressbook` take.
fn path(home: &Url, id: &str) -> String {
    format!("{}/{id}/", home.path().trim_end_matches('/'))
}

/// The origin of the home-set, which is what `server` names.
///
/// The collection is stored as an absolute path, so its root has to be
/// the authority actually serving it rather than the context root
/// discovery started from, which may sit elsewhere.
fn origin(mut url: Url) -> String {
    url.set_path("");
    url.set_query(None);
    url.set_fragment(None);

    url.to_string()
}

/// The hooks a generated calendar account fires: a notification for every
/// component the calendar advertises.
///
/// A hook naming a component it does not hold is refused when the watch
/// starts, and a calendar advertising none accepts any, so both are
/// written there.
fn caldav_hook(components: &[String]) -> CaldavHookConfig {
    let holds = |component: &str| {
        components.is_empty()
            || components
                .iter()
                .any(|held| held.eq_ignore_ascii_case(component))
    };

    CaldavHookConfig {
        on_event_added: holds(VEVENT).then(|| notify("New event in $calendar")),
        on_task_added: holds(VTODO).then(|| notify("New task in $calendar")),
        ..Default::default()
    }
}

/// The hook a generated addressbook account fires.
fn carddav_hook() -> CarddavHookConfig {
    CarddavHookConfig {
        on_card_added: Some(notify("New contact in $addressbook")),
        ..Default::default()
    }
}

/// A notification naming the item it fired for, which is all a DAV poll
/// knows: it reads etags, never the event or the card itself.
fn notify(summary: &str) -> ItemHook {
    ItemHook {
        notify: Some(NotifyConfig {
            summary: summary.to_string(),
            body: String::from("$id"),
        }),
        cmd: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_collection_is_recorded_as_an_absolute_path() {
        let home = Url::parse("https://dav.example.org/calendars/alice/").expect("parse the home");

        assert_eq!(path(&home, "work"), "/calendars/alice/work/");
    }

    #[test]
    fn the_server_is_the_authority_serving_the_home_set() {
        let url = Url::parse("https://dav.example.org/calendars/alice/").expect("parse the url");

        assert_eq!(origin(url), "https://dav.example.org/");
    }

    #[test]
    fn a_collection_reads_as_its_display_name_and_falls_back_to_its_id() {
        assert_eq!(label(Some("Work"), "work-1"), "Work (work-1)");
        assert_eq!(label(Some("  "), "work-1"), "work-1");
        assert_eq!(label(None, "work-1"), "work-1");
    }

    #[test]
    fn a_calendar_only_hooks_the_components_it_holds() {
        let events = caldav_hook(&[String::from("VEVENT")]);
        assert!(events.on_event_added.is_some());
        assert!(events.on_task_added.is_none());

        let tasks = caldav_hook(&[String::from("vtodo")]);
        assert!(tasks.on_event_added.is_none());
        assert!(tasks.on_task_added.is_some());

        // NOTE: a calendar declaring no restriction accepts any component.
        let both = caldav_hook(&[]);
        assert!(both.on_event_added.is_some());
        assert!(both.on_task_added.is_some());
    }
}
