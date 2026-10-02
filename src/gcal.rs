//! # Google Calendar
//!
//! The Google Calendar backend: one calendar watched through
//! `events.list` and its sync token.
//!
//! The first listing enumerates the calendar, one item per lone event or
//! series, and hands back a sync token; every later one only what moved,
//! a deleted event arriving as cancelled. An expired token (HTTP 410)
//! enumerates again, read against the picture. An event's etag moves on
//! every edit, which is what tells an edit apart from a feed naming an
//! event it did not change.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Error, Result, anyhow};
use io_gcal::v3::{
    client::{GcalClientStd, GcalClientStdError},
    rest::{
        calendar_list::list::GcalCalendarListListParams,
        events::{GcalEvent, GcalEventStatus, list::GcalEventsListParams},
    },
    send::GCAL_API_BASE,
};
use log::{debug, trace};
use pimalaya_config::secret::SecretResolver;
use pimalaya_stream::stream::{Stream, TlsConnectOptions};
use secrecy::ExposeSecret;
use url::Url;

use crate::{
    config::{GcalConfig, ProxyConfig},
    event::{ItemSummary, WatchDomain, WatchEvent},
    picture::{self, Item, Known},
    poll,
};

/// How long the watch waits between two polls.
const POLL_INTERVAL: Duration = Duration::from_secs(60);
/// How long a read waits before the connection is given up.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// How many events one listing page asks for, the API's ceiling.
const EVENT_PAGE: u32 = 2500;
/// The calendar every account has, which needs no listing to find.
const PRIMARY: &str = "primary";

/// Opens a connection to the Calendar API, resolving the token through
/// `resolver`.
///
/// The stream is opened here rather than by io-gcal, whose own connect
/// takes no proxy.
pub fn open(config: &GcalConfig, resolver: &mut SecretResolver) -> Result<GcalClientStd> {
    let url = Url::parse(GCAL_API_BASE)?;
    let host = url
        .host_str()
        .ok_or_else(|| anyhow!("The Calendar API URL `{url}` has no host"))?;

    let alpn = config
        .alpn
        .clone()
        .unwrap_or_else(|| vec![String::from("http/1.1")]);
    let opts = TlsConnectOptions {
        tls: config.tls.clone().into_tls(alpn),
        proxy: ProxyConfig::resolve(config.proxy.clone(), resolver)?,
        ..Default::default()
    };

    let stream = Stream::connect_tls(host, url.port().unwrap_or(443), opts)?;
    stream.set_read_timeout(Some(READ_TIMEOUT))?;

    let token = resolver.resolve(config.auth.token.clone())?;

    debug!("opened gcal connection");

    Ok(GcalClientStd::new(stream, token.expose_secret()))
}

/// Opens the connection, resolves the calendar and reads one page of it,
/// which proves the transport, the token and that the calendar exists.
///
/// The read is what proves the token: `primary` resolves with no request.
pub fn probe(config: &GcalConfig, resolver: &mut SecretResolver) -> Result<()> {
    let mut client = open(config, resolver)?;
    let id = resolve_calendar(&mut client, &config.calendar)?;
    let params = GcalEventsListParams {
        max_results: Some(1),
        ..Default::default()
    };

    client
        .events_list(&id, &params)
        .context("cannot read the watched calendar")?;

    Ok(())
}

/// Watches the configured calendar by polling its sync token, until
/// `shutdown` is set.
pub fn watch(
    config: &GcalConfig,
    interval: Option<Duration>,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let interval = interval.unwrap_or(POLL_INTERVAL);
    let mut client = open(config, &mut SecretResolver::new())?;
    let mut watched = Watched::arm(&mut client, &config.calendar)?;

    while !shutdown.load(Ordering::SeqCst) {
        if !poll::sleep(interval, shutdown) {
            break;
        }

        // NOTE: the connection sat idle the whole interval, which a
        // server is free to close, and the token may have expired
        // meanwhile, so a failed round is given a fresh connection and a
        // fresh token before the session is given up.
        if let Err(err) = watched.round(&mut client, &mut on_event) {
            debug!("gcal round failed, reconnecting: {err:#}");
            client = open(config, &mut SecretResolver::new())?;
            watched.round(&mut client, &mut on_event)?;
        }
    }

    Ok(())
}

/// The calendar the watch holds a picture of.
struct Watched {
    /// The calendar id every call speaks.
    id: String,
    /// What the calendar holds, as of `token`.
    known: Known,
    /// The sync token the next round reads from.
    token: String,
}

impl Watched {
    /// Resolves the calendar and reads what it holds, which is what a
    /// later change is a change against.
    fn arm(client: &mut GcalClientStd, name: &str) -> Result<Self> {
        let id = resolve_calendar(client, name)?;
        let (events, token) = list(client, &id, None)?;
        let known = events.into_iter().filter_map(entry).collect::<Known>();

        debug!(
            "watching gcal calendar `{name}` with {} events",
            known.len()
        );

        Ok(Self { id, known, token })
    }

    /// Reads what moved since the token, reports it, and advances it.
    ///
    /// Nothing is reported until every page the round reads has answered,
    /// so a round failing part way leaves the picture and the token where
    /// they were, and can simply be run again.
    fn round(
        &mut self,
        client: &mut GcalClientStd,
        on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
    ) -> Result<()> {
        let domain = WatchDomain::Event;

        let events = match list(client, &self.id, Some(&self.token)) {
            Ok((events, token)) => {
                trace!("gcal changed events: {}", events.len());
                self.token = token;
                self.apply(events)
            }
            // NOTE: an expired token means the server's history no longer
            // reaches that far, so the calendar is enumerated again and
            // read against the picture, the gap reporting only what
            // actually differs.
            Err(err) if is_expired(&err) => {
                debug!("gcal sync token expired, enumerating again");
                let (events, token) = list(client, &self.id, None)?;
                self.token = token;
                let fresh = events.into_iter().filter_map(entry).collect();
                picture::rebase(&mut self.known, domain, fresh)
            }
            Err(err) => return Err(err),
        };

        for event in events {
            on_event(event, None);
        }

        Ok(())
    }

    /// Reads changed events against the picture, a cancelled one being a
    /// deletion.
    fn apply(&mut self, events: Vec<GcalEvent>) -> Vec<WatchEvent> {
        let domain = WatchDomain::Event;
        let mut reported = Vec::new();

        for event in events {
            let Some(id) = event.id.clone() else {
                continue;
            };

            if event.status == Some(GcalEventStatus::Cancelled) {
                reported.extend(picture::remove(&mut self.known, domain, id));
                continue;
            }

            let item = Item {
                version: event.etag,
                ..Default::default()
            };
            reported.extend(picture::touch(&mut self.known, domain, id, item));
        }

        reported
    }
}

/// Reads one listing to its end, every page of it, from `token` or from
/// scratch, with the token the next round reads from.
fn list(
    client: &mut GcalClientStd,
    calendar: &str,
    token: Option<&str>,
) -> Result<(Vec<GcalEvent>, String)> {
    let mut events = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let params = GcalEventsListParams {
            max_results: Some(EVENT_PAGE),
            page_token: page_token.as_deref(),
            sync_token: token,
            ..Default::default()
        };
        let page = client
            .events_list(calendar, &params)
            .context("cannot list the watched calendar")?
            .response;

        events.extend(page.items);

        match (page.next_page_token, page.next_sync_token) {
            (Some(next), _) => page_token = Some(next),
            (None, Some(next)) => return Ok((events, next)),
            (None, None) => return Err(anyhow!("The Calendar API returned no sync token")),
        }
    }
}

/// What the picture keeps of a listed event, none for a cancelled one: a
/// full listing names only what stands.
fn entry(event: GcalEvent) -> Option<(String, Item)> {
    if event.status == Some(GcalEventStatus::Cancelled) {
        return None;
    }

    let item = Item {
        version: event.etag,
        ..Default::default()
    };

    Some((event.id?, item))
}

/// Resolves a calendar by its summary, case-insensitively, or by id,
/// `primary` needing no listing.
fn resolve_calendar(client: &mut GcalClientStd, name: &str) -> Result<String> {
    if name == PRIMARY {
        return Ok(String::from(PRIMARY));
    }

    calendars(client)?
        .into_iter()
        .find(|(id, summary)| id == name || summary.eq_ignore_ascii_case(name))
        .map(|(id, _)| id)
        .ok_or_else(|| anyhow!("Calendar `{name}` not found on Google Calendar"))
}

/// Lists the account's calendars, each by id and by the summary it shows
/// under, the account's own override first.
pub fn calendars(client: &mut GcalClientStd) -> Result<Vec<(String, String)>> {
    let mut calendars = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        let params = GcalCalendarListListParams {
            page_token: page_token.as_deref(),
            ..Default::default()
        };
        let page = client
            .calendar_list_list(&params)
            .context("cannot list google calendars")?
            .response;

        calendars.extend(page.items.into_iter().filter_map(|entry| {
            let summary = entry.summary_override.or(entry.summary).unwrap_or_default();
            Some((entry.id?, summary))
        }));

        match page.next_page_token {
            Some(next) => page_token = Some(next),
            None => return Ok(calendars),
        }
    }
}

/// Whether a failure is the Calendar API refusing an expired sync token.
fn is_expired(err: &Error) -> bool {
    err.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<GcalClientStdError>(),
            Some(GcalClientStdError::Send(send)) if send.is_sync_token_expired()
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::gcal::*;

    fn event(id: &str, etag: &str, status: Option<GcalEventStatus>) -> GcalEvent {
        GcalEvent {
            id: Some(String::from(id)),
            etag: Some(String::from(etag)),
            status,
            ..Default::default()
        }
    }

    #[test]
    fn a_cancelled_event_is_a_deletion_and_an_etag_move_an_edit() {
        let mut watched = Watched {
            id: String::from(PRIMARY),
            known: [event("A", "1", None), event("B", "1", None)]
                .into_iter()
                .filter_map(entry)
                .collect(),
            token: String::new(),
        };

        let events = watched.apply(vec![
            event("A", "2", Some(GcalEventStatus::Confirmed)),
            event("B", "1", Some(GcalEventStatus::Cancelled)),
            event("C", "1", None),
            // NOTE: a cancelled instance of a series the picture never
            // held is not news.
            event("D", "1", Some(GcalEventStatus::Cancelled)),
        ]);

        let domain = WatchDomain::Event;
        assert_eq!(
            vec![
                WatchEvent::ItemChanged {
                    domain,
                    id: String::from("A"),
                },
                WatchEvent::ItemRemoved {
                    domain,
                    id: String::from("B"),
                },
                WatchEvent::ItemAdded {
                    domain,
                    id: String::from("C"),
                },
            ],
            events
        );
    }
}
