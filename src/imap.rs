//! IMAP backend: session opening, the mailbox watch, and the envelope
//! resolution a hook may ask for.
//!
//! The watch is io-imap's `ImapMailboxWatch`, which holds IDLE and
//! reports UID-keyed deltas, named by the server under QRESYNC and
//! diffed locally without it. This module only translates them, so
//! carillon owns no watcher.
//!
//! A delta names a UID, never a subject, so [`Resolver`] reads the
//! envelope on a second connection.

use std::{
    collections::BTreeSet,
    fmt,
    io::{self, Read, Write},
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::RecvTimeoutError,
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use io_imap::{
    client::{ImapClientStd, ImapMailboxWatchStream, ImapMailboxWatchStreamOptions, default_port},
    coroutine::{ImapCoroutine, ImapCoroutineState, ImapYield},
    rfc3501::{
        examine::ImapMailboxExamine,
        fetch::{ImapMessageFetch, ImapMessageFetchOptions},
    },
    session::ImapSessionOpenOptions,
    types::{
        core::NString,
        envelope::Address,
        fetch::{MacroOrMessageDataItemNames, MessageDataItem, MessageDataItemName},
        flag::Flag,
        mailbox::Mailbox,
        response::Capability,
        sequence::SequenceSet,
    },
    watch::ImapMailboxWatchEvent,
};
use io_sasl::mechanism::Sasl;
use log::{debug, trace};
use pimalaya_stream::tls::Tls;
use url::Url;

use crate::{
    config::{ImapConfig, resolve_auto_id_params},
    event::{ItemSummary, WatchDomain, WatchEvent},
};

/// How long a watch waits for an event before checking the shutdown
/// flag again.
const POLL_TICK: Duration = Duration::from_millis(500);
/// How long a polling watch waits between two re-reads, unless the
/// config says otherwise.
const POLL_INTERVAL: Duration = Duration::from_secs(60);
/// How long the watch worker, and the resolver, may sit in a read
/// before looking at the shutdown flag again.
const READ_TIMEOUT: Duration = Duration::from_secs(1);
/// Per-read scratch buffer for the resolver. Only envelopes are read,
/// never a body.
const READ_BUF: usize = 8 * 1024;

/// Opens an authenticated IMAP session, returning the client and the
/// capabilities the handshake reported.
///
/// Every protocol decision (transport from the scheme, STARTTLS
/// ordering, the `ID` quirk, the SASL-IR policy) belongs to io-imap's
/// session coroutine; this only resolves the config into its inputs.
pub fn open(config: &ImapConfig) -> Result<(ImapClientStd, Vec<Capability<'static>>)> {
    let mut tls: Tls = config.tls.clone().into();
    tls.rustls.alpn = vec![String::from("imap")];

    let server = parse_server(&config.server)?;
    let sasl: Option<Sasl> = config
        .sasl
        .clone()
        .map(|sasl| {
            let host = server
                .host_str()
                .ok_or_else(|| anyhow!("Cannot derive host from IMAP server `{server}`"))?;
            // NOTE: url does not know the imap(s) default ports, so fall
            // back to the same scheme defaults io-imap connects with.
            let port = server.port().unwrap_or(default_port(server.scheme()));
            sasl.try_into_sasl(host, port)
        })
        .transpose()?;

    let opts = ImapSessionOpenOptions {
        starttls: config.starttls,
        auto_id: resolve_auto_id_params(&config.id)?,
        sasl_ir: config.sasl_ir,
    };

    debug!("opening imap session");
    trace!("server: {server}");

    Ok(ImapClientStd::connect(&server, &tls, sasl, opts)?)
}

/// Watches `collection` over a held IDLE, until the connection ends or
/// `shutdown` is set.
///
/// io-imap runs the watch on its own thread and hands events over a
/// channel, so this loop stays free to notice a shutdown between them.
pub fn watch_idle(
    config: &ImapConfig,
    collection: &str,
    timeout: Option<Duration>,
    shutdown: &Arc<AtomicBool>,
    on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    watch(config, collection, timeout, None, shutdown, on_event)
}

/// Watches `collection` the same way, but re-reading on an interval
/// instead of holding IDLE.
///
/// For a server whose IDLE cannot be trusted: it costs a re-read per
/// interval and notices a change that much later, which is why it is
/// asked for rather than fallen back on.
pub fn watch_poll(
    config: &ImapConfig,
    collection: &str,
    interval: Option<Duration>,
    shutdown: &Arc<AtomicBool>,
    on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let interval = interval.unwrap_or(POLL_INTERVAL);

    watch(config, collection, None, Some(interval), shutdown, on_event)
}

fn watch(
    config: &ImapConfig,
    collection: &str,
    idle_timeout: Option<Duration>,
    poll: Option<Duration>,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let (client, capability) = open(config)?;
    let watched = Mailbox::try_from(collection.to_string())
        .map_err(|err| anyhow!("Invalid mailbox name `{collection}`: {err}"))?;
    let opts = ImapMailboxWatchStreamOptions {
        shutdown_poll: READ_TIMEOUT,
        idle_timeout,
        poll,
    };
    let stream = client.watch_mailbox(watched, &capability, opts)?;

    let result = drain(&stream, shutdown, &mut on_event);
    // NOTE: close winds the worker down cleanly, whether we are leaving
    // because of a shutdown or because the connection failed.
    let closed = stream.close();

    result.or_else(|err| closed.map_err(Into::into).and(Err(err)))
}

/// Drains the watch stream into `on_event` until shutdown or failure.
fn drain(
    stream: &ImapMailboxWatchStream,
    shutdown: &Arc<AtomicBool>,
    on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    while !shutdown.load(Ordering::SeqCst) {
        match stream.recv_timeout(POLL_TICK) {
            Ok(Ok(event)) => {
                for event in translate(event) {
                    on_event(event, None);
                }
            }
            Ok(Err(err)) => return Err(err).context("imap watch failed"),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

/// Maps one io-imap delta onto carillon's vocabulary.
///
/// A flag delta names every flag that moved at once, and a hook fires
/// for one flag, so a delta of several becomes several events.
fn translate(event: ImapMailboxWatchEvent) -> Vec<WatchEvent> {
    match event {
        ImapMailboxWatchEvent::EnvelopeAdded { uid, .. } => vec![WatchEvent::ItemAdded {
            domain: WatchDomain::Message,
            id: uid.to_string(),
        }],
        ImapMailboxWatchEvent::EnvelopeRemoved { uid } => vec![WatchEvent::ItemRemoved {
            domain: WatchDomain::Message,
            id: uid.to_string(),
        }],
        ImapMailboxWatchEvent::FlagsAdded { uid, flags } => render_flags(&flags)
            .into_iter()
            .map(|flag| WatchEvent::FlagAdded {
                domain: WatchDomain::Message,
                id: uid.to_string(),
                flag,
            })
            .collect(),
        ImapMailboxWatchEvent::FlagsRemoved { uid, flags } => render_flags(&flags)
            .into_iter()
            .map(|flag| WatchEvent::FlagRemoved {
                domain: WatchDomain::Message,
                id: uid.to_string(),
                flag,
            })
            .collect(),
    }
}

/// Renders IMAP flags as the strings a hook filter matches against.
fn render_flags(flags: &[Flag<'static>]) -> BTreeSet<String> {
    flags.iter().map(|flag| flag.to_string()).collect()
}

/// A second connection, opened lazily, that turns a UID into the
/// envelope a notification wants.
///
/// The watch connection is busy holding IDLE, so resolving rides its
/// own session. It opens on the first arrival a hook asks about, and
/// re-opens once if the server dropped it in between.
///
/// The coroutines are pumped here rather than through the client's own
/// blocking runner, so that a shutdown is noticed between reads: a
/// resolve against a server that stopped answering must not be what
/// holds a Ctrl+C.
pub struct Resolver<'a> {
    config: &'a ImapConfig,
    mailbox: &'a str,
    shutdown: &'a Arc<AtomicBool>,
    client: Option<ImapClientStd>,
}

impl<'a> Resolver<'a> {
    /// Prepares a resolver; no connection is opened until something is
    /// resolved.
    pub fn new(config: &'a ImapConfig, mailbox: &'a str, shutdown: &'a Arc<AtomicBool>) -> Self {
        Self {
            config,
            mailbox,
            shutdown,
            client: None,
        }
    }

    /// Fetches the envelope of `uid`, reconnecting once when the
    /// pooled session turns out to be dead.
    pub fn summary(&mut self, uid: &str) -> Result<ItemSummary> {
        match self.fetch(uid) {
            Ok(summary) => Ok(summary),
            Err(err) if self.shutdown.load(Ordering::SeqCst) => Err(err),
            Err(err) => {
                debug!("resolver session lost, reconnecting: {err:#}");
                self.client = None;
                self.fetch(uid)
            }
        }
    }

    fn fetch(&mut self, uid: &str) -> Result<ItemSummary> {
        let mailbox = Mailbox::try_from(self.mailbox.to_string())
            .map_err(|err| anyhow!("Invalid mailbox name `{}`: {err}", self.mailbox))?;

        let parsed = uid
            .parse::<u32>()
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or_else(|| anyhow!("Invalid UID `{uid}`"))?;

        if self.client.is_none() {
            let (mut client, _capability) = open(self.config)?;
            // NOTE: the same arrangement the watch worker makes: a read
            // deadline to be woken up by, and no retry strategy to
            // swallow the wakeup, so the loop below can look at the
            // shutdown flag between reads.
            client.stream.set_read_timeout(Some(READ_TIMEOUT))?;
            client.stream.stop_retrying();
            self.client = Some(client);
        }

        // NOTE: unwrapping is safe, the client was just opened.
        let client = self.client.as_mut().unwrap();
        let shutdown = self.shutdown;

        let examine = ImapMailboxExamine::new(mailbox, Default::default());
        pump(client, examine, shutdown)?;

        let sequence = SequenceSet::from(parsed..=parsed);
        let items =
            MacroOrMessageDataItemNames::MessageDataItemNames(vec![MessageDataItemName::Envelope]);
        let opts = ImapMessageFetchOptions {
            uid: true,
            modifiers: Vec::new(),
        };
        let fetched = pump(
            client,
            ImapMessageFetch::new(sequence, items, opts),
            shutdown,
        )?;

        let items = fetched
            .into_values()
            .next()
            .ok_or_else(|| anyhow!("No envelope returned for UID `{uid}`"))?;

        Ok(summarize(items.into_inner()))
    }
}

/// Runs one coroutine over the resolver's connection, checking the
/// shutdown flag between reads.
///
/// This is what the client's own runner does, minus the part where a
/// silent server can hold the thread for as long as the transport
/// allows: here a read deadline expires into another look at the flag.
fn pump<C, T, E>(
    client: &mut ImapClientStd,
    mut coroutine: C,
    shutdown: &Arc<AtomicBool>,
) -> Result<T>
where
    C: ImapCoroutine<Yield = ImapYield, Return = Result<T, E>>,
    E: fmt::Display,
{
    let mut buf = [0u8; READ_BUF];
    let mut arg: Option<Vec<u8>> = None;

    loop {
        match coroutine.resume(&mut client.fragmentizer, arg.take().as_deref()) {
            ImapCoroutineState::Yielded(ImapYield::WantsRead) => loop {
                if shutdown.load(Ordering::SeqCst) {
                    bail!("Shutting down");
                }

                match client.stream.read(&mut buf) {
                    Ok(0) => bail!("Connection closed by peer"),
                    Ok(read) => {
                        arg = Some(buf[..read].to_vec());
                        break;
                    }
                    Err(err) if is_timeout(&err) => continue,
                    Err(err) => return Err(err).context("read failed"),
                }
            },
            ImapCoroutineState::Yielded(ImapYield::WantsWrite(bytes)) => {
                client.stream.write_all(&bytes).context("write failed")?;
            }
            ImapCoroutineState::Complete(Ok(value)) => return Ok(value),
            ImapCoroutineState::Complete(Err(err)) => bail!("{err}"),
        }
    }
}

/// Whether an I/O error is the read deadline expiring, which is a
/// wakeup rather than a failure.
fn is_timeout(err: &io::Error) -> bool {
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
    )
}

/// Folds a FETCH response into what the hook templates expose.
fn summarize(items: Vec<MessageDataItem<'static>>) -> ItemSummary {
    let mut summary = ItemSummary::default();

    for item in items {
        let MessageDataItem::Envelope(envelope) = item else {
            continue;
        };

        summary.subject = nstring(&envelope.subject);
        summary.date = nstring(&envelope.date);

        if let Some(from) = envelope.from.first() {
            summary.from_name = nstring(&from.name);
            summary.from_addr = address(from);
        }

        if let Some(to) = envelope.to.first() {
            summary.to_name = nstring(&to.name);
            summary.to_addr = address(to);
        }
    }

    summary
}

/// Renders an address as `mailbox@host`, when both parts are present.
fn address(address: &Address<'static>) -> Option<String> {
    let mailbox = nstring(&address.mailbox)?;
    let host = nstring(&address.host)?;
    Some(format!("{mailbox}@{host}"))
}

/// Reads an IMAP NString as a plain string, dropping NIL and empties.
fn nstring(value: &NString<'static>) -> Option<String> {
    let value = value.0.as_ref()?;
    let value = String::from_utf8_lossy(value.as_ref()).trim().to_string();
    (!value.is_empty()).then_some(value)
}

/// Parses an IMAP server string into a URL.
///
/// Accepts a bare authority (`imap.example.org[:port]`), treated as
/// `imaps://<authority>` so a portless value stays secure, or a full
/// URL with an `imap://`, `imaps://` or `unix://` scheme used verbatim.
pub fn parse_server(server: &str) -> Result<Url> {
    match Url::parse(server) {
        Ok(url) => Ok(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            Ok(Url::parse(&format!("imaps://{server}"))?)
        }
        Err(err) => Err(err.into()),
    }
}
