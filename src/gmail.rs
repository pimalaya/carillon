//! # Gmail
//!
//! The Gmail backend: one label watched through `users.history.list`.
//!
//! The watch keeps the history cursor and asks what moved since, scoped
//! to the label. A message added with the label, or the label added to a
//! message, is an arrival; deleted, or the label taken off, a removal.
//! `UNREAD` and `STARRED` are the shared `Seen` (inverted) and `Flagged`.
//!
//! io-gmail ships a poll coroutine of its own, which keeps its cursor
//! private: a connection lost mid-poll could only be recovered by a new
//! baseline, losing what moved in between. The rounds here run the same
//! requests on the client, so a lost connection is reopened and the
//! round run again from the same cursor.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Error, Result, anyhow};
use io_gmail::v1::{
    client::{GmailClientStd, GmailClientStdConnectOptions, GmailClientStdError},
    rest::{
        history::{
            GmailHistory,
            list::{GmailHistoryList, GmailHistoryListParams},
        },
        messages::{GmailMessage, GmailMessageFormat},
    },
};
use log::{debug, trace, warn};
use pimalaya_config::secret::SecretResolver;
use secrecy::ExposeSecret;

use crate::{
    config::{GmailConfig, ProxyConfig},
    event::{ItemSummary, WatchDomain, WatchEvent},
    poll,
};

/// How long the watch waits between two polls.
const POLL_INTERVAL: Duration = Duration::from_secs(60);
/// The headers an arrival's envelope is read from.
const ENVELOPE_HEADERS: &[&str] = &["Subject", "From", "To", "Date"];
/// The system label whose absence is the shared `Seen`.
const UNREAD: &str = "UNREAD";
/// The system label that is the shared `Flagged`.
const STARRED: &str = "STARRED";

/// Opens a connection to the Gmail API, resolving the token through
/// `resolver`.
pub fn open(config: &GmailConfig, resolver: &mut SecretResolver) -> Result<GmailClientStd> {
    let alpn = config
        .alpn
        .clone()
        .unwrap_or_else(|| vec![String::from("http/1.1")]);
    let opts = GmailClientStdConnectOptions {
        tls: config.tls.clone().into_tls(alpn),
        proxy: ProxyConfig::resolve(config.proxy.clone(), resolver)?,
        user_id: config.user_id.clone().unwrap_or_else(|| String::from("me")),
    };
    let token = resolver.resolve(config.auth.token.clone())?;

    debug!("opening gmail connection");

    Ok(GmailClientStd::connect(token.expose_secret(), opts)?)
}

/// Opens the connection and resolves the label, which proves the
/// transport, the token and that the label exists.
pub fn probe(config: &GmailConfig, resolver: &mut SecretResolver) -> Result<()> {
    let mut client = open(config, resolver)?;
    resolve_label(&mut client, &config.mailbox)?;

    Ok(())
}

/// Watches the configured label by polling its history, until `shutdown`
/// is set.
pub fn watch(
    config: &GmailConfig,
    interval: Option<Duration>,
    resolve: bool,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let interval = interval.unwrap_or(POLL_INTERVAL);
    let mut client = open(config, &mut SecretResolver::new())?;
    let mut watched = Watched::arm(&mut client, &config.mailbox)?;

    while !shutdown.load(Ordering::SeqCst) {
        if !poll::sleep(interval, shutdown) {
            break;
        }

        // NOTE: the connection sat idle the whole interval, which a
        // server is free to close, and the token may have expired
        // meanwhile, so a failed round is given a fresh connection and a
        // fresh token before the session is given up.
        if let Err(err) = watched.round(&mut client, resolve, &mut on_event) {
            debug!("gmail round failed, reconnecting: {err:#}");
            client = open(config, &mut SecretResolver::new())?;
            watched.round(&mut client, resolve, &mut on_event)?;
        }
    }

    Ok(())
}

/// The label the watch holds, and how far its history was read.
struct Watched {
    /// The label id every call speaks.
    label: String,
    /// The history cursor the next round reads from.
    history: String,
}

impl Watched {
    /// Resolves the label and reads the cursor changes are read from.
    fn arm(client: &mut GmailClientStd, name: &str) -> Result<Self> {
        let label = resolve_label(client, name)?;
        let history = current_history(client)?;

        debug!("watching gmail label `{name}` from history {history}");

        Ok(Self { label, history })
    }

    /// Reads what moved since the cursor, reports it, and advances it.
    ///
    /// Nothing is reported until every request the round makes has
    /// answered, so a round failing part way leaves the cursor where it
    /// was, and can simply be run again.
    fn round(
        &mut self,
        client: &mut GmailClientStd,
        resolve: bool,
        on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
    ) -> Result<()> {
        let (records, next) = match self.history(client) {
            Ok(read) => read,
            // NOTE: a cursor older than Gmail keeps (about a week) is
            // answered 404, and nothing can say what moved in between:
            // the watch resumes from now rather than guessing.
            Err(err) if is_expired(&err) => {
                warn!("gmail history expired, resuming from now");
                self.history = current_history(client)?;
                return Ok(());
            }
            Err(err) => return Err(err),
        };

        trace!("gmail history records: {}", records.len());

        let events = self.events(&records);
        let mut reported = Vec::with_capacity(events.len());

        for event in events {
            let summary = match &event {
                WatchEvent::ItemAdded { id, .. } if resolve => summary(client, id),
                _ => None,
            };
            reported.push((event, summary));
        }

        for (event, summary) in reported {
            on_event(event, summary);
        }

        self.history = next;

        Ok(())
    }

    /// Reads every history page since the cursor, with the cursor the
    /// next round reads from.
    fn history(&self, client: &mut GmailClientStd) -> Result<(Vec<GmailHistory>, String)> {
        let mut records = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let params = GmailHistoryListParams {
                start_history_id: &self.history,
                label_id: Some(&self.label),
                page_token: page_token.as_deref(),
                ..Default::default()
            };
            let list = GmailHistoryList::new(&client.auth, &client.user_id, &params)?;
            let page = client
                .run(list)
                .context("cannot read the gmail history")?
                .response;

            records.extend(page.history);

            match page.next_page_token {
                Some(token) => page_token = Some(token),
                None => {
                    let next = page.history_id.unwrap_or_else(|| self.history.clone());
                    return Ok((records, next));
                }
            }
        }
    }

    /// Reads history records as the events they stand for.
    fn events(&self, records: &[GmailHistory]) -> Vec<WatchEvent> {
        let domain = WatchDomain::Message;
        let mut events = Vec::new();

        for record in records {
            for added in &record.messages_added {
                if added.message.label_ids.contains(&self.label) {
                    let id = added.message.id.clone();
                    events.push(WatchEvent::ItemAdded { domain, id });
                }
            }

            for deleted in &record.messages_deleted {
                let id = deleted.message.id.clone();
                events.push(WatchEvent::ItemRemoved { domain, id });
            }

            for change in &record.labels_added {
                for label in &change.label_ids {
                    events.extend(self.label_event(&change.message, label, true));
                }
            }

            for change in &record.labels_removed {
                for label in &change.label_ids {
                    events.extend(self.label_event(&change.message, label, false));
                }
            }
        }

        events
    }

    /// What a label set on or taken off a message stands for.
    fn label_event(&self, message: &GmailMessage, label: &str, set: bool) -> Option<WatchEvent> {
        let domain = WatchDomain::Message;
        let id = message.id.clone();

        if label == self.label {
            return Some(if set {
                WatchEvent::ItemAdded { domain, id }
            } else {
                WatchEvent::ItemRemoved { domain, id }
            });
        }

        // NOTE: Gmail says what is unread, the shared vocabulary what is
        // seen, so `UNREAD` reads the other way round.
        let (flag, added) = match label {
            UNREAD => ("Seen", !set),
            STARRED => ("Flagged", set),
            _ => return None,
        };
        let flag = String::from(flag);

        Some(if added {
            WatchEvent::FlagAdded { domain, id, flag }
        } else {
            WatchEvent::FlagRemoved { domain, id, flag }
        })
    }
}

/// Reads an arrival's envelope from its headers, or nothing when the
/// message is already gone.
fn summary(client: &mut GmailClientStd, id: &str) -> Option<ItemSummary> {
    match client.message_get(id, GmailMessageFormat::Metadata, ENVELOPE_HEADERS) {
        Ok(out) => Some(summarize(&out.response)),
        Err(err) => {
            warn!("cannot read the envelope of gmail message `{id}`: {err}");
            None
        }
    }
}

/// Folds a message's headers into what an arrival hook templates on.
fn summarize(message: &GmailMessage) -> ItemSummary {
    let header = |name| {
        message
            .payload
            .as_ref()
            .and_then(|payload| payload.header(name))
    };

    let (from_name, from_addr) = header("From").map(party).unwrap_or_default();
    let to = header("To").and_then(|to| to.split(',').next());
    let (to_name, to_addr) = to.map(party).unwrap_or_default();

    ItemSummary {
        from_name,
        from_addr,
        to_name,
        to_addr,
        subject: header("Subject").map(String::from),
        date: header("Date").map(String::from),
    }
}

/// Splits a `Name <address>` header value into its name and address.
fn party(value: &str) -> (Option<String>, Option<String>) {
    let Some((name, address)) = value.rsplit_once('<') else {
        let address = value.trim();
        return (None, (!address.is_empty()).then(|| address.to_string()));
    };

    let name = name.trim().trim_matches('"').trim();
    let address = address.trim_end().trim_end_matches('>').trim();

    (
        (!name.is_empty()).then(|| name.to_string()),
        (!address.is_empty()).then(|| address.to_string()),
    )
}

/// Resolves a label by name, case-insensitively, or by id.
fn resolve_label(client: &mut GmailClientStd, name: &str) -> Result<String> {
    let listed = client
        .labels_list()
        .context("cannot list gmail labels")?
        .response;

    listed
        .labels
        .into_iter()
        .find(|label| label.id == name || label.name.eq_ignore_ascii_case(name))
        .map(|label| label.id)
        .ok_or_else(|| anyhow!("Label `{name}` not found on Gmail"))
}

/// The history cursor as it stands now.
fn current_history(client: &mut GmailClientStd) -> Result<String> {
    client
        .profile_get()
        .context("cannot read the gmail profile")?
        .response
        .history_id
        .ok_or_else(|| anyhow!("Gmail returned no history cursor"))
}

/// Whether a failure is Gmail refusing a cursor too old (HTTP 404).
fn is_expired(err: &Error) -> bool {
    err.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<GmailClientStdError>(),
            Some(GmailClientStdError::Send(send)) if send.status() == Some(404)
        )
    })
}

#[cfg(test)]
mod tests {
    use io_gmail::v1::rest::history::{GmailHistoryLabel, GmailHistoryMessage};

    use crate::gmail::*;

    fn watched() -> Watched {
        Watched {
            label: String::from("Label_7"),
            history: String::from("1"),
        }
    }

    fn message(id: &str, labels: &[&str]) -> GmailMessage {
        GmailMessage {
            id: String::from(id),
            label_ids: labels.iter().map(|label| String::from(*label)).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn the_watched_label_moving_is_an_arrival_or_a_removal() {
        let record = GmailHistory {
            id: String::from("2"),
            messages: Vec::new(),
            messages_deleted: Vec::new(),
            labels_added: Vec::new(),
            messages_added: vec![
                GmailHistoryMessage {
                    message: message("A", &["Label_7"]),
                },
                GmailHistoryMessage {
                    message: message("B", &["INBOX"]),
                },
            ],
            labels_removed: vec![GmailHistoryLabel {
                message: message("C", &[]),
                label_ids: vec![String::from("Label_7")],
            }],
        };

        let events = watched().events(&[record]);
        let domain = WatchDomain::Message;

        assert_eq!(
            vec![
                WatchEvent::ItemAdded {
                    domain,
                    id: String::from("A"),
                },
                WatchEvent::ItemRemoved {
                    domain,
                    id: String::from("C"),
                },
            ],
            events
        );
    }

    /// Gmail says what is unread, so reading a message clears a label.
    #[test]
    fn unread_and_starred_are_the_shared_flags() {
        let watched = watched();
        let read = watched.label_event(&message("M", &[]), UNREAD, false);
        let starred = watched.label_event(&message("M", &[]), STARRED, true);
        let other = watched.label_event(&message("M", &[]), "IMPORTANT", true);

        assert!(
            matches!(read, Some(WatchEvent::FlagAdded { ref flag, .. }) if flag == "Seen"),
            "got {read:?}"
        );
        assert!(
            matches!(starred, Some(WatchEvent::FlagAdded { ref flag, .. }) if flag == "Flagged"),
            "got {starred:?}"
        );
        assert!(other.is_none());
    }

    #[test]
    fn a_header_party_splits_into_name_and_address() {
        assert_eq!(
            (
                Some(String::from("Alice Doe")),
                Some(String::from("alice@example.org"))
            ),
            party("\"Alice Doe\" <alice@example.org>")
        );
        assert_eq!(
            (None, Some(String::from("bob@example.org"))),
            party("bob@example.org")
        );
    }
}
