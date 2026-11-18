//! # Gmail
//!
//! The Gmail backend: one label watched through `users.history.list`.
//!
//! The watch keeps the history cursor and asks what moved since, scoped
//! to the label. A message added with the label, or the label added to a
//! message, is an arrival; deleted, or the label taken off, a removal.
//! `UNREAD` and `STARRED` are the shared `Seen` (inverted) and `Flagged`.
//!
//! io-gmail ships a poll coroutine of its own, which reads every arrival
//! and groups what a tick saw by kind. The rounds here run the same
//! requests on the client: an envelope is read only when a hook wants
//! it, the events keep the order of the history, and a lost connection
//! is reopened and the round run again from the same cursor. The cursor
//! outlives a lost session, the next one reading what moved meanwhile.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use io_gmail::v1::{
    client::{GmailClientStd, GmailClientStdConnectOptions, GmailClientStdError},
    history_poll::GMAIL_HISTORY_POLL_MAX_PAGES,
    rest::{
        history::{
            GmailHistory,
            list::{GmailHistoryList, GmailHistoryListParams},
        },
        messages::{GmailMessage, GmailMessageFormat},
    },
    send::GmailSendError,
};
use log::{debug, trace, warn};
use mime::{codec::rfc2047, tree::field::address::parse_addresses};
use pimalaya_config::secret::SecretResolver;
use secrecy::ExposeSecret;

use crate::{
    config::{GmailConfig, ProxyConfig},
    error::{self, ErrorCode, GoogleAnswer, google_code},
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
///
/// `cursor` is how far the previous watch of the account read, which
/// this one reads from at once, and where it leaves its own.
///
/// `on_open` is called once the session is up and its collection read,
/// right before the first wait.
pub fn watch(
    config: &GmailConfig,
    interval: Option<Duration>,
    resolve: bool,
    cursor: &mut Option<String>,
    shutdown: &Arc<AtomicBool>,
    on_open: impl FnOnce(),
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let interval = interval.unwrap_or(POLL_INTERVAL);
    let mut client = open(config, &mut SecretResolver::new())?;
    let mut watched = Watched::arm(&mut client, &config.mailbox, cursor.as_deref())?;
    let mut wait = cursor.is_none();

    *cursor = Some(watched.history.clone());
    on_open();

    while !shutdown.load(Ordering::SeqCst) {
        if wait && !poll::sleep(interval, shutdown) {
            break;
        }

        wait = true;

        // NOTE: the connection sat idle the whole interval, which a
        // server is free to close, and the token may have expired
        // meanwhile, so a failed round is given a fresh connection and a
        // fresh token before the session is given up.
        if let Err(err) = watched.round(&mut client, resolve, &mut on_event) {
            debug!("gmail round failed, reconnecting: {err:#}");
            client = open(config, &mut SecretResolver::new())?;
            watched.round(&mut client, resolve, &mut on_event)?;
        }

        *cursor = Some(watched.history.clone());
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
    /// Resolves the label and takes the cursor changes are read from:
    /// `cursor` when a previous watch left one, the current one otherwise.
    fn arm(client: &mut GmailClientStd, name: &str, cursor: Option<&str>) -> Result<Self> {
        let label = resolve_label(client, name)?;
        let history = match cursor {
            Some(cursor) => String::from(cursor),
            None => current_history(client)?,
        };

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
        // NOTE: nothing can say what moved since a cursor that cannot be
        // read from: the watch resumes from now rather than guessing.
        let Some((records, next)) = self.history(client)? else {
            self.history = current_history(client)?;
            return Ok(());
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
    /// next round reads from, or nothing when the cursor cannot be read
    /// from: expired, or listing without an end.
    fn history(&self, client: &mut GmailClientStd) -> Result<Option<(Vec<GmailHistory>, String)>> {
        let mut records = Vec::new();
        let mut page_token: Option<String> = None;

        for _ in 0..GMAIL_HISTORY_POLL_MAX_PAGES {
            let params = GmailHistoryListParams {
                start_history_id: &self.history,
                label_id: Some(&self.label),
                page_token: page_token.as_deref(),
                ..Default::default()
            };
            let list = GmailHistoryList::new(&client.auth, &client.user_id, &params)?;
            let page = match client.run(list) {
                Ok(out) => out.response,
                // NOTE: a cursor older than Gmail keeps (about a week).
                Err(GmailClientStdError::Send(err)) if err.is_history_expired() => {
                    warn!("gmail history expired, resuming from now");
                    return Ok(None);
                }
                Err(err) => return Err(err).context("cannot read the gmail history"),
            };

            records.extend(page.history);

            match page.next_page_token {
                Some(token) if page_token.as_ref() != Some(&token) => page_token = Some(token),
                Some(_) => break,
                None => {
                    let next = page.history_id.filter(|id| !id.is_empty());
                    let next = next.unwrap_or_else(|| self.history.clone());
                    return Ok(Some((records, next)));
                }
            }
        }

        warn!("gmail history lists without an end, resuming from now");

        Ok(None)
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
    let (to_name, to_addr) = header("To").map(party).unwrap_or_default();
    let subject =
        header("Subject").map(|subject| rfc2047::decode_text(subject.as_bytes()).into_owned());

    ItemSummary {
        from_name,
        from_addr,
        to_name,
        to_addr,
        subject,
        date: header("Date").map(String::from),
    }
}

/// Reads the first mailbox of an address list header into its name, RFC
/// 2047 encoded words decoded, and its address (RFC 5322 §3.4).
fn party(value: &str) -> (Option<String>, Option<String>) {
    let list = parse_addresses(value.as_bytes());
    let Some(mailbox) = list.first() else {
        return (None, None);
    };

    let name = mailbox.name.clone().filter(|name| !name.is_empty());
    (name, mailbox.address.clone())
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
        .ok_or_else(|| {
            let message = format!("Label `{name}` not found on Gmail");
            error::not_found(WatchDomain::Message, message)
        })
}

/// The history cursor as it stands now.
fn current_history(client: &mut GmailClientStd) -> Result<String> {
    client
        .profile_get()
        .context("cannot read the gmail profile")?
        .response
        .history_id
        .filter(|id| !id.is_empty())
        .ok_or_else(|| anyhow!("Gmail returned no history cursor"))
}

/// The code of a Gmail refusal in an error chain ([`google_code`]).
pub(crate) fn refusal_code(cause: &(dyn std::error::Error + 'static)) -> Option<ErrorCode> {
    let GmailClientStdError::Send(GmailSendError::Api(err)) =
        cause.downcast_ref::<GmailClientStdError>()?
    else {
        return None;
    };

    google_code(&GoogleAnswer {
        status: err.status,
        message: &err.message,
        reasons: &err.reasons,
        detail_reasons: &err.detail_reasons,
        google_status: err.google_status.as_deref(),
    })
}

#[cfg(test)]
mod tests {
    use io_gmail::v1::rest::history::{GmailHistoryLabel, GmailHistoryMessage};

    use crate::{
        gmail::*,
        scripted::{OK, Scripted},
    };

    fn client(bodies: &[&'static str]) -> GmailClientStd {
        let answers: Vec<_> = bodies.iter().map(|body| (OK, *body)).collect();
        answering(&answers)
    }

    fn answering(answers: &[(&'static str, &'static str)]) -> GmailClientStd {
        let stream = Scripted::answering(answers);

        GmailClientStd::new(stream, "token", Default::default())
    }

    fn seen(client: &mut GmailClientStd) -> Vec<String> {
        let stream = client.stream.as_any_mut().downcast_mut::<Scripted>();
        stream.unwrap().seen.clone()
    }

    fn watched() -> Watched {
        Watched {
            label: String::from("Label_7"),
            history: String::from("1"),
        }
    }

    #[test]
    fn a_watch_left_a_cursor_reads_what_moved_since() {
        let mut client = client(&[
            r#"{"labels":[{"id":"Label_7","name":"Work"}]}"#,
            r#"{"history":[{"id":"43","messagesAdded":[{"message":{"id":"A","threadId":"A","labelIds":["Label_7"]}}]}],"historyId":"44"}"#,
        ]);

        let mut watched = Watched::arm(&mut client, "work", Some("42")).unwrap();
        let mut events = Vec::new();
        watched
            .round(&mut client, false, &mut |event, _| events.push(event))
            .unwrap();

        assert_eq!(
            vec![WatchEvent::ItemAdded {
                domain: WatchDomain::Message,
                id: String::from("A"),
            }],
            events
        );
        assert_eq!("44", watched.history);

        let seen = seen(&mut client);
        assert_eq!(2, seen.len(), "{seen:?}");
        assert!(seen[1].contains("startHistoryId=42"), "{seen:?}");
    }

    #[test]
    fn a_history_listing_without_an_end_resumes_from_now() {
        let page = r#"{"history":[],"nextPageToken":"P","historyId":"50"}"#;
        let profile = r#"{"emailAddress":"alice@example.org","historyId":"60"}"#;
        let mut client = client(&[page, page, profile]);

        let mut watched = watched();
        let mut events = Vec::new();
        watched
            .round(&mut client, false, &mut |event, _| events.push(event))
            .unwrap();

        assert!(events.is_empty(), "got {events:?}");
        assert_eq!("60", watched.history);

        let seen = seen(&mut client);
        assert_eq!(3, seen.len(), "{seen:?}");
        assert!(seen[2].contains("/profile"), "{seen:?}");
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

    #[test]
    fn a_to_of_several_addresses_reads_as_its_first_and_names_decode() {
        let message: GmailMessage = serde_json::from_str(
            r#"{"id":"A","payload":{"headers":[
                {"name":"Subject","value":"=?UTF-8?Q?R=C3=A9union?="},
                {"name":"From","value":"=?ISO-8859-1?Q?Andr=E9?= <andre@example.org>"},
                {"name":"To","value":"\"Doe, Jane\" <jane@example.org>, bob@example.org"}
            ]}}"#,
        )
        .unwrap();

        let summary = summarize(&message);

        assert_eq!(Some("Réunion"), summary.subject.as_deref());
        assert_eq!(Some("André"), summary.from_name.as_deref());
        assert_eq!(Some("andre@example.org"), summary.from_addr.as_deref());
        assert_eq!(Some("Doe, Jane"), summary.to_name.as_deref());
        assert_eq!(Some("jane@example.org"), summary.to_addr.as_deref());
    }

    /// The failure a watch meets arming against a single answer.
    fn refused(status: &'static str, body: &'static str) -> anyhow::Error {
        let mut client = answering(&[(status, body)]);
        Watched::arm(&mut client, "work", None).err().unwrap()
    }

    #[test]
    fn a_403_for_missing_scopes_is_permission_denied() {
        let err = refused(
            "403 Forbidden",
            r#"{"error":{"code":403,"message":"Request had insufficient authentication scopes.","errors":[{"message":"Insufficient Permission","domain":"global","reason":"insufficientPermissions"}],"status":"PERMISSION_DENIED"}}"#,
        );

        assert_eq!(
            Some(ErrorCode::PermissionDenied),
            crate::error::code(&err),
            "{err:#}"
        );
    }

    #[test]
    fn a_429_or_a_403_naming_a_rate_limit_is_rate_limited() {
        let throttled = refused(
            "429 Too Many Requests",
            r#"{"error":{"code":429,"message":"Too many concurrent requests for user","errors":[{"message":"Too many concurrent requests for user","domain":"global","reason":"rateLimitExceeded"}],"status":"RESOURCE_EXHAUSTED"}}"#,
        );
        let limited = refused(
            "403 Forbidden",
            r#"{"error":{"code":403,"message":"User Rate Limit Exceeded","errors":[{"message":"User Rate Limit Exceeded","domain":"usageLimits","reason":"userRateLimitExceeded"}]}}"#,
        );

        assert_eq!(
            Some(ErrorCode::RateLimited),
            crate::error::code(&throttled),
            "{throttled:#}"
        );
        assert_eq!(
            Some(ErrorCode::RateLimited),
            crate::error::code(&limited),
            "{limited:#}"
        );
    }

    /// A label the account lists under no name nor id is the mailbox
    /// not found.
    #[test]
    fn a_label_the_account_lacks_is_mailbox_not_found() {
        let mut client = client(&[r#"{"labels":[{"id":"Label_7","name":"Work"}]}"#]);
        let err = resolve_label(&mut client, "Archive").unwrap_err();

        let code = crate::error::code(&err);
        assert_eq!(Some(ErrorCode::MailboxNotFound), code, "{err:#}");
    }
}
