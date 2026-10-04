//! # Microsoft Graph
//!
//! The Microsoft Graph backend: one token and one connection watching up
//! to a mail folder, a contact folder and a calendar.
//!
//! Mail and contacts read a delta query scoped to their folder: the first
//! round enumerates the folder and hands back a delta link, every later
//! one only what moved, an item leaving the folder arriving as a removal.
//! An expired link (HTTP 410) enumerates again, read against the picture.
//!
//! The calendar reads no delta. Graph only offers one over a calendar
//! view, whose window is fixed at the first round, so an event leaving it
//! would read as a deletion. It lists the calendar's events with their
//! `changeKey` instead and diffs that, the way a WebDAV poll diffs etags.

use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Error, Result, bail};
use io_msgraph::v1::{
    client::{MsgraphClientStd, MsgraphClientStdConnectOptions, MsgraphClientStdError},
    rest::users::{
        calendars::list::MsgraphCalendarsListParams,
        contact_folders::list::MsgraphContactFoldersListParams,
        contacts::delta::MsgraphContactDelta,
        events::list::MsgraphEventsListParams,
        mail_folders::list::MsgraphMailFoldersListParams,
        messages::{MsgraphFlagStatus, MsgraphMessage, delta::MsgraphMessageDelta},
    },
};
use log::{debug, trace};
use pimalaya_config::secret::SecretResolver;
use secrecy::ExposeSecret;

use crate::{
    config::{MsgraphConfig, ProxyConfig},
    event::{ItemSummary, WatchDomain, WatchEvent},
    picture::{self, Item, Known},
    poll,
};

/// How long the watch waits between two polls.
const POLL_INTERVAL: Duration = Duration::from_secs(60);
/// The mail properties a delta row carries when no hook wants an
/// envelope: membership is the folder scope, so only the flags.
const MESSAGE_SELECT: &str = "isRead,flag";
/// The same, plus what an arrival hook templates on.
const ENVELOPE_SELECT: &str = "isRead,flag,subject,from,toRecipients,receivedDateTime";
/// The contact property a delta row carries, the id riding along anyway.
const CONTACT_SELECT: &str = "displayName";
/// The event properties a listing carries: what tells an edit apart.
const EVENT_SELECT: &str = "id,changeKey";
/// How many calendars or contact folders one listing page asks for.
const FOLDER_PAGE: u32 = 100;
/// The id a listing of the account gives the default contact folder,
/// which Graph addresses by omitting the folder segment.
const DEFAULT_CONTACT_FOLDER: &str = "contacts";
/// How many events one listing page asks for.
const EVENT_PAGE: u32 = 250;

/// Opens a connection to the Graph API, resolving the token through
/// `resolver`.
pub fn open(config: &MsgraphConfig, resolver: &mut SecretResolver) -> Result<MsgraphClientStd> {
    let alpn = config
        .alpn
        .clone()
        .unwrap_or_else(|| vec![String::from("http/1.1")]);
    let opts = MsgraphClientStdConnectOptions {
        tls: config.tls.clone().into_tls(alpn),
        proxy: ProxyConfig::resolve(config.proxy.clone(), resolver)?,
        user_id: config.user_id.clone().unwrap_or_else(|| String::from("me")),
    };
    let token = resolver.resolve(config.auth.token.clone())?;

    debug!("opening msgraph connection");

    Ok(MsgraphClientStd::connect(token.expose_secret(), opts)?)
}

/// Opens the connection and resolves every configured collection, which
/// proves the transport, the token, its scopes and that each collection
/// exists.
pub fn probe(config: &MsgraphConfig, resolver: &mut SecretResolver) -> Result<()> {
    let mut client = open(config, resolver)?;

    for (domain, collection) in config.collections() {
        match domain {
            WatchDomain::Message => {
                resolve_mail_folder(&mut client, collection.value)?;
            }
            WatchDomain::Card => {
                resolve_contact_folder(&mut client, collection.value)?;
            }
            WatchDomain::Event => {
                resolve_calendar(&mut client, collection.value)?;
            }
            WatchDomain::Task => unreachable!("Microsoft Graph watches no task"),
        }
    }

    Ok(())
}

/// Watches every configured collection by polling, until `shutdown` is
/// set.
pub fn watch(
    config: &MsgraphConfig,
    interval: Option<Duration>,
    resolve: bool,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let interval = interval.unwrap_or(POLL_INTERVAL);
    let mut client = open(config, &mut SecretResolver::new())?;
    let mut watched = Vec::new();

    for (domain, collection) in config.collections() {
        watched.push(Watched::arm(
            &mut client,
            domain,
            collection.value,
            resolve,
        )?);
    }

    while !shutdown.load(Ordering::SeqCst) {
        if !poll::sleep(interval, shutdown) {
            break;
        }

        for watched in &mut watched {
            // NOTE: the connection sat idle the whole interval, which a
            // server is free to close, and the token may have expired
            // meanwhile, so a failed round is given a fresh connection and
            // a fresh token before the session is given up.
            if let Err(err) = watched.round(&mut client, resolve, &mut on_event) {
                debug!("msgraph round failed, reconnecting: {err:#}");
                client = open(config, &mut SecretResolver::new())?;
                watched.round(&mut client, resolve, &mut on_event)?;
            }
        }
    }

    Ok(())
}

/// One collection the watch holds a picture of, in one domain.
struct Watched {
    /// What the collection holds, which picks the endpoints it is read
    /// through and names the events it reports.
    domain: WatchDomain,
    /// The id of the folder or calendar every call speaks, `None` for
    /// the default contact folder.
    id: Option<String>,
    /// What the collection holds, as of the last round.
    known: Known,
    /// The delta link the next round reads from, mail and contacts only.
    link: Option<String>,
}

/// What a delta row says of one item, whatever its domain.
struct Row {
    /// The item's id.
    id: String,
    /// Whether the row is a removal.
    removed: bool,
    /// The item as the row reads it.
    item: Item,
    /// The envelope an arrival hook templates on, when one was asked for.
    summary: Option<ItemSummary>,
}

impl Watched {
    /// Resolves `name` into its collection and reads what it holds, which
    /// is what a later change is a change against.
    fn arm(
        client: &mut MsgraphClientStd,
        domain: WatchDomain,
        name: &str,
        resolve: bool,
    ) -> Result<Self> {
        let id = match domain {
            WatchDomain::Message => Some(resolve_mail_folder(client, name)?),
            WatchDomain::Card => resolve_contact_folder(client, name)?,
            WatchDomain::Event => Some(resolve_calendar(client, name)?),
            WatchDomain::Task => unreachable!("Microsoft Graph watches no task"),
        };

        let mut watched = Self {
            domain,
            id,
            known: Known::new(),
            link: None,
        };

        watched.known = watched.list(client, resolve)?;

        debug!(
            "watching msgraph {} `{name}` with {} items",
            domain.collection_name(),
            watched.known.len()
        );

        Ok(watched)
    }

    /// Reads what moved since the last round and reports it.
    ///
    /// Nothing is reported until every page the round reads has answered,
    /// so a round failing part way leaves the picture and the link where
    /// they were, and can simply be run again.
    fn round(
        &mut self,
        client: &mut MsgraphClientStd,
        resolve: bool,
        on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
    ) -> Result<()> {
        let rows = match self.link.clone() {
            None => {
                let fresh = self.list(client, resolve)?;
                return report(
                    picture::rebase(&mut self.known, self.domain, fresh),
                    on_event,
                );
            }
            Some(link) => match self.delta(client, Some(&link), resolve) {
                Ok((rows, next)) => {
                    self.link = Some(next);
                    rows
                }
                // NOTE: an expired link means the server's history no
                // longer reaches that far, so the folder is enumerated
                // again and read against the picture, the gap reporting
                // only what actually differs.
                Err(err) if is_expired(&err) => {
                    debug!("msgraph delta link expired, enumerating again");
                    let fresh = self.list(client, resolve)?;
                    return report(
                        picture::rebase(&mut self.known, self.domain, fresh),
                        on_event,
                    );
                }
                Err(err) => return Err(err),
            },
        };

        trace!("msgraph delta rows: {}", rows.len());

        let mut reported = Vec::new();

        for row in rows {
            if row.removed {
                reported.extend(
                    picture::remove(&mut self.known, self.domain, row.id)
                        .map(|event| (event, None)),
                );
                continue;
            }

            for event in picture::touch(&mut self.known, self.domain, row.id, row.item) {
                let summary = matches!(event, WatchEvent::ItemAdded { .. })
                    .then(|| row.summary.clone())
                    .flatten();
                reported.push((event, summary));
            }
        }

        for (event, summary) in reported {
            on_event(event, summary);
        }

        Ok(())
    }

    /// Lists what the collection holds now, which for mail and contacts
    /// also opens the delta link the next round reads from.
    fn list(&mut self, client: &mut MsgraphClientStd, resolve: bool) -> Result<Known> {
        if self.domain == WatchDomain::Event {
            return self.events(client);
        }

        let (rows, link) = self.delta(client, None, resolve)?;
        self.link = Some(link);

        Ok(rows
            .into_iter()
            .filter(|row| !row.removed)
            .map(|row| (row.id, row.item))
            .collect())
    }

    /// Reads one delta round to its end, from `link` or from the start,
    /// and hands back its rows with the link the next round reads from.
    fn delta(
        &self,
        client: &mut MsgraphClientStd,
        link: Option<&str>,
        resolve: bool,
    ) -> Result<(Vec<Row>, String)> {
        let mut rows = Vec::new();
        let mut link = link.map(String::from);

        loop {
            let (page, next, delta) = self.delta_page(client, link.as_deref(), resolve)?;
            rows.extend(page);

            match (next, delta) {
                (Some(next), _) => link = Some(next),
                (None, Some(delta)) => return Ok((rows, delta)),
                (None, None) => bail!("Microsoft Graph delta round ended with no delta link"),
            }
        }
    }

    /// Reads one delta page: the first of a round from the folder, any
    /// other from the link the previous page handed back.
    fn delta_page(
        &self,
        client: &mut MsgraphClientStd,
        link: Option<&str>,
        resolve: bool,
    ) -> Result<(Vec<Row>, Option<String>, Option<String>)> {
        let folder = self.id.as_deref();

        match self.domain {
            WatchDomain::Message => {
                let select = if resolve {
                    ENVELOPE_SELECT
                } else {
                    MESSAGE_SELECT
                };
                let page = match link {
                    Some(link) => client.messages_delta_from_link(link),
                    None => client.messages_delta(folder, Some(select)),
                }
                .context("cannot read message changes")?
                .response;

                let rows = page.value.into_iter().map(message_row).collect();
                Ok((rows, page.next_link, page.delta_link))
            }
            WatchDomain::Card => {
                let page = match link {
                    Some(link) => client.contacts_delta_from_link(link),
                    None => client.contacts_delta(folder, Some(CONTACT_SELECT)),
                }
                .context("cannot read contact changes")?
                .response;

                let rows = page.value.into_iter().map(contact_row).collect();
                Ok((rows, page.next_link, page.delta_link))
            }
            WatchDomain::Event | WatchDomain::Task => {
                unreachable!("a calendar is listed, not read through a delta")
            }
        }
    }

    /// Lists the calendar's events with the `changeKey` an edit moves.
    fn events(&self, client: &mut MsgraphClientStd) -> Result<Known> {
        let params = MsgraphEventsListParams {
            top: Some(EVENT_PAGE),
            select: Some(EVENT_SELECT),
            ..Default::default()
        };

        let mut page = client
            .events_list(self.id.as_deref(), &params)
            .context("cannot list the watched calendar")?
            .response;
        let mut known = Known::new();

        loop {
            for event in page.value {
                let item = Item {
                    version: event.change_key,
                    ..Default::default()
                };
                known.insert(event.id, item);
            }

            let Some(link) = page.next_link else {
                return Ok(known);
            };

            page = client
                .events_list_from_link(&link)
                .context("cannot list the watched calendar")?
                .response;
        }
    }
}

/// Hands every event of a rebase to the hooks, none carrying a summary:
/// an enumeration reads no envelope.
fn report(
    events: Vec<WatchEvent>,
    on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    for event in events {
        on_event(event, None);
    }

    Ok(())
}

/// Folds a message delta row into what the picture keeps of it.
///
/// `isRead` is the shared `Seen` and a follow-up flag the shared
/// `Flagged`, so a filter written for IMAP fires here too.
fn message_row(row: MsgraphMessageDelta) -> Row {
    let MsgraphMessageDelta { message, removed } = row;
    let mut flags = BTreeSet::new();

    if message.is_read == Some(true) {
        flags.insert(String::from("Seen"));
    }

    let status = message.flag.as_ref().and_then(|flag| flag.flag_status);

    if status == Some(MsgraphFlagStatus::Flagged) {
        flags.insert(String::from("Flagged"));
    }

    // NOTE: the envelope properties are selected only when a hook wants
    // them, so a row carrying a subject or a sender is one that asked.
    let summary =
        (message.subject.is_some() || message.from.is_some()).then(|| summarize(&message));

    Row {
        id: message.id,
        removed: removed.is_some(),
        item: Item {
            flags,
            version: None,
        },
        summary,
    }
}

/// Folds a contact delta row into what the picture keeps of it.
fn contact_row(row: MsgraphContactDelta) -> Row {
    Row {
        id: row.contact.id,
        removed: row.removed.is_some(),
        item: Item::default(),
        summary: None,
    }
}

/// Folds a message into what an arrival hook templates on.
fn summarize(message: &MsgraphMessage) -> ItemSummary {
    let from = message.from.as_ref().map(|from| &from.email_address);
    let to = message.to_recipients.first().map(|to| &to.email_address);

    ItemSummary {
        from_name: from.and_then(|from| from.name.clone()),
        from_addr: from.and_then(|from| from.address.clone()),
        to_name: to.and_then(|to| to.name.clone()),
        to_addr: to.and_then(|to| to.address.clone()),
        subject: message.subject.clone(),
        date: message.received_date_time.clone(),
    }
}

/// Resolves a mail folder by display name or id among the top-level
/// ones, then as a well-known name or a nested folder id.
fn resolve_mail_folder(client: &mut MsgraphClientStd, name: &str) -> Result<String> {
    let listed = client
        .mail_folders_list(&MsgraphMailFoldersListParams::default())
        .context("cannot list mail folders")?
        .response;

    let found = listed
        .value
        .into_iter()
        .find(|folder| folder.id == name || folder.display_name.eq_ignore_ascii_case(name));

    if let Some(folder) = found {
        return Ok(folder.id);
    }

    // NOTE: `inbox`, `sentitems` and the other well-known names are not
    // display names, which are localized, and a nested folder is not
    // top-level, so both are asked for directly.
    let folder = client
        .mail_folder_get(name)
        .with_context(|| format!("Mail folder `{name}` not found on Microsoft Graph"))?
        .response;

    Ok(folder.id)
}

/// Resolves a contact folder by display name or id, `Contacts` naming the
/// default one, which Graph does not list.
///
/// `contacts` is the id a listing of the account gives the default
/// folder, so it names that one before any folder of that name. A
/// top-level folder is found by id or display name, and any other id is
/// then asked for directly, which reaches a nested folder too.
fn resolve_contact_folder(client: &mut MsgraphClientStd, name: &str) -> Result<Option<String>> {
    if name == DEFAULT_CONTACT_FOLDER {
        return Ok(None);
    }

    let params = MsgraphContactFoldersListParams {
        top: Some(FOLDER_PAGE),
        ..Default::default()
    };
    let listed = client
        .contact_folders_list(&params)
        .context("cannot list contact folders")?
        .response;

    let found = listed
        .value
        .into_iter()
        .find(|folder| folder.id == name || folder.display_name.eq_ignore_ascii_case(name));

    if let Some(folder) = found {
        return Ok(Some(folder.id));
    }

    if name.eq_ignore_ascii_case(DEFAULT_CONTACT_FOLDER) {
        return Ok(None);
    }

    let folder = client
        .contact_folder_get(name)
        .with_context(|| format!("Contact folder `{name}` not found on Microsoft Graph"))?
        .response;

    Ok(Some(folder.id))
}

/// Resolves a calendar by id, the one a listing of the account gives, or
/// by name, then asks for an id the first listing page did not hold.
fn resolve_calendar(client: &mut MsgraphClientStd, name: &str) -> Result<String> {
    let params = MsgraphCalendarsListParams {
        top: Some(FOLDER_PAGE),
        ..Default::default()
    };
    let listed = client
        .calendars_list(&params)
        .context("cannot list calendars")?
        .response;

    let found = listed.value.into_iter().find(|calendar| {
        calendar.id == name
            || calendar
                .name
                .as_deref()
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(name))
    });

    if let Some(calendar) = found {
        return Ok(calendar.id);
    }

    let calendar = client
        .calendar_get(name)
        .with_context(|| format!("Calendar `{name}` not found on Microsoft Graph"))?
        .response;

    Ok(calendar.id)
}

/// Whether a failure is Graph refusing an expired delta link (HTTP 410).
fn is_expired(err: &Error) -> bool {
    err.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<MsgraphClientStdError>(),
            Some(MsgraphClientStdError::Send(send)) if send.status() == Some(410)
        )
    })
}

#[cfg(test)]
mod tests {
    use io_msgraph::v1::rest::users::{
        contacts::delta::MsgraphRemoved,
        messages::{MsgraphEmailAddress, MsgraphFollowupFlag, MsgraphRecipient},
    };

    use crate::msgraph::*;

    fn message(id: &str) -> MsgraphMessage {
        MsgraphMessage {
            id: String::from(id),
            ..Default::default()
        }
    }

    #[test]
    fn read_and_follow_up_are_the_shared_flags() {
        let mut flagged = message("M");
        flagged.is_read = Some(true);
        flagged.flag = Some(MsgraphFollowupFlag {
            flag_status: Some(MsgraphFlagStatus::Flagged),
        });

        let row = message_row(MsgraphMessageDelta {
            message: flagged,
            removed: None,
        });
        let flags: Vec<&str> = row.item.flags.iter().map(String::as_str).collect();
        assert_eq!(vec!["Flagged", "Seen"], flags);

        // NOTE: a completed follow-up is no longer flagged.
        let mut done = message("M");
        done.flag = Some(MsgraphFollowupFlag {
            flag_status: Some(MsgraphFlagStatus::Complete),
        });
        let row = message_row(MsgraphMessageDelta {
            message: done,
            removed: None,
        });
        assert!(row.item.flags.is_empty());
    }

    #[test]
    fn a_removed_row_is_a_removal_and_an_envelope_rides_only_when_asked() {
        let row = message_row(MsgraphMessageDelta {
            message: message("M"),
            removed: Some(MsgraphRemoved::default()),
        });
        assert!(row.removed);
        assert!(row.summary.is_none());

        let mut arrived = message("N");
        arrived.subject = Some(String::from("Lunch"));
        arrived.from = Some(MsgraphRecipient {
            email_address: MsgraphEmailAddress {
                name: Some(String::from("Alice")),
                address: Some(String::from("alice@example.org")),
            },
        });

        let row = message_row(MsgraphMessageDelta {
            message: arrived,
            removed: None,
        });
        let summary = row.summary.expect("an envelope");
        assert_eq!(Some(String::from("Lunch")), summary.subject);
        assert_eq!(Some(String::from("Alice")), summary.from_name);
        assert_eq!(Some(String::from("alice@example.org")), summary.from_addr);
    }
}
