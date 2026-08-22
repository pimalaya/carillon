//! IMAP backend: session opening, the mailbox watch, and envelope
//! resolution for the hooks that want one.
//!
//! The watch itself is io-imap's: [`ImapMailboxWatch`] holds IDLE,
//! re-reads what changed on every wake and reports UID-keyed deltas.
//! With QRESYNC the server names them; without it, io-imap re-reads the
//! mailbox and diffs locally. Either way this module only translates
//! those deltas into [`WatchEvent`], so mirador owns no watcher of its
//! own.
//!
//! A delta names a UID, never a subject. [`Resolver`] fetches the
//! envelope of an arrival on a second connection, and only when a hook
//! declares it wants one.

use std::{
    collections::BTreeSet,
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use io_imap::{
    client::{ImapClient, ImapClientStd, ImapMailboxWatchStream, default_port},
    rfc3501::fetch::ImapMessageFetchOptions,
    session::ImapSessionOpenOptions,
    types::{
        core::NString,
        envelope::Address,
        fetch::{MacroOrMessageDataItemNames, MessageDataItem, MessageDataItemName},
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
    event::{MessageSummary, WatchEvent},
};

/// How long a watch waits for an event before checking the shutdown
/// flag again.
const POLL_TICK: Duration = Duration::from_millis(500);

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
                .ok_or_else(|| anyhow!("cannot derive host from IMAP server `{server}`"))?;
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

/// Watches `mailbox` until the connection ends or `shutdown` is set,
/// calling `on_event` for every change.
///
/// io-imap runs the watch on its own thread and hands events over a
/// channel, so this loop stays free to notice a shutdown between them.
pub fn watch(
    config: &ImapConfig,
    mailbox: &str,
    shutdown: &Arc<AtomicBool>,
    mut on_event: impl FnMut(WatchEvent),
) -> Result<()> {
    let (client, capability) = open(config)?;
    let watched = Mailbox::try_from(mailbox.to_string())
        .map_err(|err| anyhow!("invalid mailbox name `{mailbox}`: {err}"))?;
    let stream = client.watch_mailbox(watched, &capability)?;

    let result = pump(&stream, shutdown, &mut on_event);
    // NOTE: close winds the worker down cleanly, whether we are leaving
    // because of a shutdown or because the connection failed.
    let closed = stream.close();

    result.or_else(|err| closed.map_err(Into::into).and(Err(err)))
}

/// Drains the watch stream into `on_event` until shutdown or failure.
fn pump(
    stream: &ImapMailboxWatchStream,
    shutdown: &Arc<AtomicBool>,
    on_event: &mut impl FnMut(WatchEvent),
) -> Result<()> {
    use std::sync::mpsc::RecvTimeoutError;

    while !shutdown.load(Ordering::SeqCst) {
        match stream.recv_timeout(POLL_TICK) {
            Ok(Ok(event)) => {
                if let Some(event) = translate(event) {
                    on_event(event);
                }
            }
            Ok(Err(err)) => return Err(err).context("imap watch failed"),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

/// Maps one io-imap delta onto mirador's vocabulary.
fn translate(event: ImapMailboxWatchEvent) -> Option<WatchEvent> {
    let event = match event {
        ImapMailboxWatchEvent::EnvelopeAdded { uid, .. } => WatchEvent::MessageAdded {
            id: uid.to_string(),
        },
        ImapMailboxWatchEvent::EnvelopeRemoved { uid } => WatchEvent::MessageRemoved {
            id: uid.to_string(),
        },
        ImapMailboxWatchEvent::FlagsAdded { uid, flags } => WatchEvent::FlagsAdded {
            id: uid.to_string(),
            flags: render_flags(&flags),
        },
        ImapMailboxWatchEvent::FlagsRemoved { uid, flags } => WatchEvent::FlagsRemoved {
            id: uid.to_string(),
            flags: render_flags(&flags),
        },
    };

    Some(event)
}

/// Renders IMAP flags as the strings a hook filter matches against.
fn render_flags(flags: &[io_imap::types::flag::Flag<'static>]) -> BTreeSet<String> {
    flags.iter().map(|flag| flag.to_string()).collect()
}

/// A second connection, opened lazily, that turns a UID into the
/// envelope a notification wants.
///
/// The watch connection is busy holding IDLE, so resolving rides its
/// own session. It opens on the first arrival a hook asks about, and
/// re-opens once if the server dropped it in between.
pub struct Resolver<'a> {
    config: &'a ImapConfig,
    mailbox: &'a str,
    client: Option<ImapClientStd>,
}

impl<'a> Resolver<'a> {
    /// Prepares a resolver; no connection is opened until something is
    /// resolved.
    pub fn new(config: &'a ImapConfig, mailbox: &'a str) -> Self {
        Self {
            config,
            mailbox,
            client: None,
        }
    }

    /// Fetches the envelope of `uid`, reconnecting once when the
    /// pooled session turns out to be dead.
    pub fn summary(&mut self, uid: &str) -> Result<MessageSummary> {
        match self.fetch(uid) {
            Ok(summary) => Ok(summary),
            Err(err) => {
                debug!("resolver session lost, reconnecting: {err:#}");
                self.client = None;
                self.fetch(uid)
            }
        }
    }

    fn fetch(&mut self, uid: &str) -> Result<MessageSummary> {
        let mailbox = Mailbox::try_from(self.mailbox.to_string())
            .map_err(|err| anyhow!("invalid mailbox name `{}`: {err}", self.mailbox))?;

        if self.client.is_none() {
            let (client, _capability) = open(self.config)?;
            self.client = Some(client);
        }

        // NOTE: unwrapping is safe, the client was just opened.
        let client = self.client.as_mut().unwrap();
        client.examine(mailbox, Default::default())?;

        let parsed = uid
            .parse::<u32>()
            .ok()
            .and_then(NonZeroU32::new)
            .ok_or_else(|| anyhow!("invalid UID `{uid}`"))?;
        let sequence = SequenceSet::from(parsed..=parsed);
        let items =
            MacroOrMessageDataItemNames::MessageDataItemNames(vec![MessageDataItemName::Envelope]);
        let opts = ImapMessageFetchOptions {
            uid: true,
            modifiers: Vec::new(),
        };
        let fetched = client.fetch(sequence, items, opts)?;

        let items = fetched
            .into_values()
            .next()
            .ok_or_else(|| anyhow!("no envelope returned for UID `{uid}`"))?;

        Ok(summarize(items.into_inner()))
    }
}

/// Folds a FETCH response into what the hook templates expose.
fn summarize(items: Vec<MessageDataItem<'static>>) -> MessageSummary {
    let mut summary = MessageSummary::default();

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
