//! # IMAP
//!
//! The IMAP backend: session opening, the mailbox watch, and the envelope
//! resolution a hook may ask for.
//!
//! The watch is io-imap's `ImapMailboxWatch`, which holds IDLE and reports
//! UID-keyed deltas, named by the server under QRESYNC and diffed locally
//! without it. This module only translates them, so carillon owns no
//! watcher. What a watch knew when its connection was lost is handed to
//! the next one, which reports what moved meanwhile.
//!
//! A delta names a UID, never a subject, so [`Resolver`] reads the
//! envelope on a second connection.

use std::{
    collections::BTreeSet,
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
    client::{
        ImapClientError, ImapClientStd, ImapClientStdConnectOptions, ImapMailboxWatchStream,
        ImapMailboxWatchStreamOptions, default_port,
    },
    coroutine::{ImapCoroutine, ImapCoroutineState, ImapYield},
    rfc3501::{
        examine::ImapMailboxExamine,
        fetch::{ImapMessageFetchFirst, ImapMessageFetchOptions},
    },
    session::{ImapSessionOpenOptions, default_alpn},
    types::{
        core::NString,
        envelope::Address,
        fetch::{MacroOrMessageDataItemNames, MessageDataItem, MessageDataItemName},
        flag::Flag,
        mailbox::Mailbox,
        response::{Capability, Code},
    },
    watch::{ImapMailboxWatchEvent, ImapMailboxWatchState},
};
use io_sasl::mechanism::Sasl;
use log::{debug, trace};
use pimalaya_config::secret::SecretResolver;
use url::Url;

use crate::{
    config::{ImapConfig, ProxyConfig, resolve_auto_id_params},
    error::ErrorCode,
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
/// Per-read scratch buffer for the resolver, which reads envelopes only.
const READ_BUF: usize = 8 * 1024;

/// Opens an authenticated session, with the capabilities the handshake
/// reported.
///
/// Every protocol decision (transport from the scheme, STARTTLS ordering,
/// the `ID` quirk, the SASL-IR policy) belongs to io-imap's session
/// coroutine; this only resolves the config into its inputs.
///
/// The credential is resolved through `resolver`, so a caller opening
/// several backends of one account spawns each distinct credential
/// command once.
pub fn open(
    config: &ImapConfig,
    resolver: &mut SecretResolver,
) -> Result<(ImapClientStd, Vec<Capability<'static>>)> {
    let alpn = config.alpn.clone().unwrap_or_else(default_alpn);
    let tls = config.tls.clone().into_tls(alpn);

    let server = parse_server(&config.server)?;
    let sasl: Option<Sasl> = config
        .sasl
        .clone()
        .map(|sasl| {
            let host = server
                .host_str()
                .ok_or_else(|| anyhow!("Cannot derive host from IMAP server `{server}`"))?;
            // NOTE: url knows no imap(s) default port, so the fallback is
            // the scheme default io-imap connects with.
            let port = server.port().unwrap_or(default_port(server.scheme()));
            sasl.try_into_sasl(host, port, resolver)
        })
        .transpose()?;

    let opts = ImapClientStdConnectOptions {
        tls,
        proxy: ProxyConfig::resolve(config.proxy.clone(), resolver)?,
        sasl,
        session: ImapSessionOpenOptions {
            starttls: config.starttls,
            auto_id: resolve_auto_id_params(&config.id)?,
            sasl_ir: config.sasl_ir,
            ..Default::default()
        },
    };

    debug!("opening imap session");
    trace!("server: {server}");

    Ok(ImapClientStd::connect(&server, opts)?)
}

/// Watches `collection` over a held IDLE, until the connection ends or
/// `shutdown` is set.
///
/// io-imap runs the watch on its own thread and hands events over a
/// channel, so this loop stays free to notice a shutdown between them.
///
/// `resume` holds what the previous watch of the account knew, taken to
/// resume from and left for the next one when the connection is lost.
///
/// `on_open` is called once the session is authenticated and its mailbox
/// examined, right before the first wait.
pub fn watch_idle(
    config: &ImapConfig,
    collection: &str,
    timeout: Option<Duration>,
    resume: &mut Option<ImapMailboxWatchState>,
    shutdown: &Arc<AtomicBool>,
    on_open: impl FnOnce(),
    on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    watch(
        config,
        collection,
        Wait::Idle(timeout),
        resume,
        shutdown,
        on_open,
        on_event,
    )
}

/// Watches `collection` the same way, re-reading on an interval instead
/// of holding IDLE.
///
/// For a server whose IDLE cannot be trusted: it costs a re-read per
/// interval and notices a change that much later, which is why it is
/// asked for rather than fallen back on.
///
/// `on_open` is called once the session is authenticated and its mailbox
/// examined, right before the first wait.
pub fn watch_poll(
    config: &ImapConfig,
    collection: &str,
    interval: Option<Duration>,
    resume: &mut Option<ImapMailboxWatchState>,
    shutdown: &Arc<AtomicBool>,
    on_open: impl FnOnce(),
    on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let interval = interval.unwrap_or(POLL_INTERVAL);

    watch(
        config,
        collection,
        Wait::Poll(interval),
        resume,
        shutdown,
        on_open,
        on_event,
    )
}

/// How a watch waits for the server between two reads of its mailbox.
enum Wait {
    /// Holds IDLE, re-issued after the timeout given, io-imap's own
    /// default when `None`.
    Idle(Option<Duration>),
    /// Re-reads the mailbox on the interval given.
    Poll(Duration),
}

/// Opens the session, drains its watch stream, and closes it either way.
///
/// `on_open` is called once the session is connected and authenticated
/// and io-imap's worker examined the mailbox, before the first wait.
fn watch(
    config: &ImapConfig,
    collection: &str,
    wait: Wait,
    resume: &mut Option<ImapMailboxWatchState>,
    shutdown: &Arc<AtomicBool>,
    on_open: impl FnOnce(),
    mut on_event: impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<()> {
    let (client, capability) = open(config, &mut SecretResolver::new())?;
    let watched = Mailbox::try_from(collection.to_string())
        .map_err(|err| anyhow!("Invalid mailbox name `{collection}`: {err}"))?;

    if resume.is_some() {
        debug!("resuming the imap watch from what the last one knew");
    }

    let (idle_timeout, poll) = match wait {
        Wait::Idle(timeout) => (timeout, None),
        Wait::Poll(interval) => (None, Some(interval)),
    };
    let opts = ImapMailboxWatchStreamOptions {
        resume_from: resume.take(),
        shutdown_poll: READ_TIMEOUT,
        idle_timeout,
        poll,
    };
    let stream = client.watch_mailbox(watched, &capability, opts)?;

    // NOTE: a worker ending before its EXAMINE opened the mailbox leaves
    // its error on the stream, which the drain hands back.
    while !shutdown.load(Ordering::SeqCst) {
        match stream.wait_opened(POLL_TICK) {
            Ok(()) => {
                on_open();
                break;
            }
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    let result = drain(&stream, shutdown, &mut on_event);
    // NOTE: this winds the worker down cleanly, whether the exit is a
    // shutdown or a failed connection.
    let state = stream.into_state();

    match result {
        Ok(()) => *resume = state?,
        // NOTE: a refusal takes a new baseline: a mailbox recreated
        // meanwhile would refuse the state of the old one forever.
        Err(err) => {
            if err.is_connection_lost() {
                *resume = state.ok().flatten();
            }

            return Err(err).context("imap watch failed");
        }
    }

    Ok(())
}

/// Drains the watch stream into `on_event` until shutdown or failure.
fn drain(
    stream: &ImapMailboxWatchStream,
    shutdown: &Arc<AtomicBool>,
    on_event: &mut impl FnMut(WatchEvent, Option<ItemSummary>),
) -> Result<(), ImapClientError> {
    while !shutdown.load(Ordering::SeqCst) {
        match stream.recv_timeout(POLL_TICK) {
            Ok(Ok(event)) => {
                for event in translate(event) {
                    on_event(event, None);
                }
            }
            Ok(Err(err)) => return Err(err),
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

/// A second connection, opened lazily, turning a UID into the envelope a
/// notification wants.
///
/// The watch connection is busy holding IDLE, so resolving rides its own,
/// opened on the first arrival a hook asks about and pumped here rather
/// than by the client's runner, so a silent server cannot hold a Ctrl+C.
pub struct Resolver<'a> {
    config: &'a ImapConfig,
    mailbox: &'a str,
    shutdown: &'a Arc<AtomicBool>,
    client: Option<ImapClientStd>,
}

impl<'a> Resolver<'a> {
    /// Prepares a resolver, opening nothing until something is resolved.
    pub fn new(config: &'a ImapConfig, mailbox: &'a str, shutdown: &'a Arc<AtomicBool>) -> Self {
        Self {
            config,
            mailbox,
            shutdown,
            client: None,
        }
    }

    /// Fetches the envelope of `uid`, reconnecting once when the session
    /// turns out to be dead.
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
            let (mut client, _capability) = open(self.config, &mut SecretResolver::new())?;
            // NOTE: the arrangement the watch worker makes: a read
            // deadline to be woken up by, and no retry strategy to swallow
            // the wakeup, so the loop below sees the shutdown flag.
            client.stream.set_read_timeout(Some(READ_TIMEOUT))?;
            client.stream.stop_retrying();
            self.client = Some(client);
        }

        // NOTE: unwrapping is safe, the client was just opened.
        let client = self.client.as_mut().unwrap();
        let shutdown = self.shutdown;

        let examine = ImapMailboxExamine::new(mailbox, Default::default());
        pump(client, examine, shutdown)?;

        let items =
            MacroOrMessageDataItemNames::MessageDataItemNames(vec![MessageDataItemName::Envelope]);
        let opts = ImapMessageFetchOptions {
            uid: true,
            modifiers: Vec::new(),
        };
        // NOTE: leaves out the FETCH of any other message a server adds.
        let fetch = ImapMessageFetchFirst::new(parsed, items, opts);
        let items = pump(client, fetch, shutdown)?;

        Ok(summarize(items.into_inner()))
    }
}

/// Runs one coroutine over the resolver's connection, checking the
/// shutdown flag between reads.
///
/// What the client's own runner does, minus the part where a silent
/// server holds the thread for as long as the transport allows: here a
/// read deadline expires into another look at the flag.
fn pump<C, T, E>(
    client: &mut ImapClientStd,
    mut coroutine: C,
    shutdown: &Arc<AtomicBool>,
) -> Result<T>
where
    C: ImapCoroutine<Yield = ImapYield, Return = Result<T, E>>,
    E: Into<ImapClientError>,
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
            ImapCoroutineState::Yielded(ImapYield::WantsTag) => {
                let tag = client.tags.generate();
                arg = Some(tag.as_ref().as_bytes().to_vec());
            }
            ImapCoroutineState::Complete(Ok(value)) => return Ok(value),
            ImapCoroutineState::Complete(Err(err)) => {
                let err: ImapClientError = err.into();
                return Err(err.into());
            }
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
/// A bare authority (`imap.example.org[:port]`) is read as
/// `imaps://<authority>`, so a portless value stays secure; an `imap://`,
/// `imaps://` or `unix://` URL is used verbatim.
pub fn parse_server(server: &str) -> Result<Url> {
    match Url::parse(server) {
        Ok(url) => Ok(url),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            Ok(Url::parse(&format!("imaps://{server}"))?)
        }
        Err(err) => Err(err.into()),
    }
}

/// The code of an IMAP refusal in an error chain: a completion naming
/// `NOPERM` is `permission-denied`, one naming `OVERQUOTA`
/// `quota-exceeded`, and one naming `NONEXISTENT`, which only a command
/// naming the watched mailbox meets, `mailbox-not-found` (RFC 5530 §3).
pub(crate) fn refusal_code(cause: &(dyn std::error::Error + 'static)) -> Option<ErrorCode> {
    let status = cause.downcast_ref::<ImapClientError>()?.status()?;

    match status.code.as_ref()? {
        Code::OverQuota => Some(ErrorCode::QuotaExceeded),
        Code::Other(code) if code.inner().eq_ignore_ascii_case(b"NOPERM") => {
            Some(ErrorCode::PermissionDenied)
        }
        Code::Other(code) if code.inner().eq_ignore_ascii_case(b"NONEXISTENT") => {
            Some(ErrorCode::MailboxNotFound)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        io::{BufRead, BufReader},
        net::{TcpListener, TcpStream},
        thread,
    };

    use pimalaya_cli::error::{ErrorReport, with_code};
    use toml::from_str;

    use crate::imap::*;

    /// Starts a server with no QRESYNC taking one connection per entry
    /// of `sessions`, a UIDVALIDITY and the UIDs it lists. Each answers
    /// one read of the mailbox, then closes on the next.
    fn serve(sessions: Vec<(u32, Vec<u32>)>) -> ImapConfig {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            for (uid_validity, uids) in sessions {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };

                session(stream, uid_validity, &uids);
            }
        });

        from_str(&format!(
            "mailbox = \"INBOX\"\nserver = \"imap://{addr}\"\n"
        ))
        .unwrap()
    }

    fn session(mut stream: TcpStream, uid_validity: u32, uids: &[u32]) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut examined = false;

        stream
            .write_all(b"* PREAUTH [CAPABILITY IMAP4rev1] ready\r\n")
            .unwrap();

        loop {
            let mut line = String::new();

            if reader.read_line(&mut line).unwrap_or(0) == 0 {
                return;
            }

            let mut words = line.split_whitespace();
            let tag = words.next().unwrap_or_default();

            let data = match words.next().unwrap_or_default() {
                "EXAMINE" if examined => return,
                "EXAMINE" => {
                    examined = true;
                    let exists = uids.len();
                    format!("* {exists} EXISTS\r\n* OK [UIDVALIDITY {uid_validity}] valid\r\n")
                }
                "UID" => uids
                    .iter()
                    .enumerate()
                    .map(|(seq, uid)| format!("* {} FETCH (UID {uid} FLAGS ())\r\n", seq + 1))
                    .collect(),
                _ => String::new(),
            };

            let answer = format!("{data}{tag} OK done\r\n");
            stream.write_all(answer.as_bytes()).unwrap();
        }
    }

    /// Runs one polling watch until its first event, answering what it
    /// ended on, the events it reported, and how many of them came before
    /// the session was reported open, `None` when it never was.
    fn poll(
        config: &ImapConfig,
        resume: &mut Option<ImapMailboxWatchState>,
    ) -> (Result<()>, Vec<WatchEvent>, Option<usize>) {
        let shutdown = Arc::new(AtomicBool::new(false));
        let events = RefCell::new(Vec::new());
        let opened = Cell::new(None);
        let interval = Some(Duration::from_millis(10));

        let on_open = || opened.set(Some(events.borrow().len()));
        let on_event = |event, _| {
            events.borrow_mut().push(event);
            shutdown.store(true, Ordering::SeqCst);
        };
        let result = watch_poll(
            config, "INBOX", interval, resume, &shutdown, on_open, on_event,
        );

        (result, events.into_inner(), opened.get())
    }

    #[test]
    fn a_watch_reopened_after_a_lost_connection_reports_what_moved_meanwhile() {
        let config = serve(vec![(1, vec![1]), (1, vec![1, 2])]);
        let mut resume = None;

        let (lost, events, _) = poll(&config, &mut resume);
        assert!(lost.is_err());
        assert!(events.is_empty(), "got {events:?}");
        assert!(resume.is_some());

        let (_, events, opened) = poll(&config, &mut resume);
        assert_eq!(Some(0), opened, "the session opens before it reports");
        assert_eq!(
            vec![WatchEvent::ItemAdded {
                domain: WatchDomain::Message,
                id: String::from("2"),
            }],
            events
        );
    }

    #[test]
    fn a_mailbox_recreated_meanwhile_takes_a_new_baseline() {
        let config = serve(vec![(1, vec![1]), (2, vec![1, 2])]);
        let mut resume = None;

        let (lost, _, _) = poll(&config, &mut resume);
        assert!(lost.is_err());
        assert!(resume.is_some());

        let (refused, events, _) = poll(&config, &mut resume);
        assert!(refused.is_err());
        assert!(events.is_empty(), "got {events:?}");
        assert!(resume.is_none());
    }

    /// Starts a server refusing every authentication, taking one
    /// connection.
    fn refuse_auth() -> ImapConfig {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream.try_clone().unwrap());

            stream
                .write_all(b"* OK [CAPABILITY IMAP4rev1 AUTH=PLAIN] ready\r\n")
                .unwrap();

            loop {
                let mut line = String::new();

                if reader.read_line(&mut line).unwrap_or(0) == 0 {
                    return;
                }

                let mut words = line.split_whitespace();
                let tag = words.next().unwrap_or_default();
                let answer = match words.next().unwrap_or_default() {
                    "AUTHENTICATE" | "LOGIN" => {
                        format!("{tag} NO [AUTHENTICATIONFAILED] refused\r\n")
                    }
                    _ => format!("{tag} OK done\r\n"),
                };

                if stream.write_all(answer.as_bytes()).is_err() {
                    return;
                }
            }
        });

        from_str(&format!(
            "mailbox = \"INBOX\"\nserver = \"imap://{addr}\"\n\
             sasl.plain.username = \"alice\"\n\
             sasl.plain.password.raw = \"wrong\"\n"
        ))
        .unwrap()
    }

    /// A session the server refuses to authenticate is never reported
    /// open, only lost.
    #[test]
    fn a_refused_authentication_never_opens_the_session() {
        let config = refuse_auth();

        let (result, events, opened) = poll(&config, &mut None);
        assert!(result.is_err());
        assert!(events.is_empty(), "got {events:?}");
        assert_eq!(None, opened);
    }

    /// Starts a server refusing every EXAMINE with the response code
    /// `code`, taking any number of connections.
    fn refuse(code: &'static str) -> ImapConfig {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                let mut reader = BufReader::new(stream.try_clone().unwrap());

                stream
                    .write_all(b"* PREAUTH [CAPABILITY IMAP4rev1] ready\r\n")
                    .unwrap();

                loop {
                    let mut line = String::new();

                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }

                    let mut words = line.split_whitespace();
                    let tag = words.next().unwrap_or_default();
                    let answer = match words.next().unwrap_or_default() {
                        "EXAMINE" => format!("{tag} NO [{code}] refused\r\n"),
                        _ => format!("{tag} OK done\r\n"),
                    };

                    if stream.write_all(answer.as_bytes()).is_err() {
                        break;
                    }
                }
            }
        });

        from_str(&format!(
            "mailbox = \"INBOX\"\nserver = \"imap://{addr}\"\n"
        ))
        .unwrap()
    }

    /// The watch's failure carries the code, and so does the failure
    /// object a command failing with it prints.
    #[test]
    fn a_watch_refused_with_noperm_is_permission_denied() {
        let config = refuse("NOPERM");

        let (result, _, _) = poll(&config, &mut None);
        let err = result.unwrap_err();
        assert_eq!(
            Some(ErrorCode::PermissionDenied),
            crate::error::code(&err),
            "{err:#}"
        );

        let report = ErrorReport::<ErrorCode>::from(with_code(err, crate::error::code));
        let report = serde_json::to_value(&report).unwrap();
        assert_eq!("permission-denied", report["code"], "{report}");
    }

    /// A mailbox the server refuses to examine fails the watch before it
    /// is ever reported open, a missing one coded so.
    #[test]
    fn a_refused_examine_never_opens_the_session() {
        let config = refuse("NONEXISTENT");

        let (result, events, opened) = poll(&config, &mut None);
        let err = result.unwrap_err();
        let code = crate::error::code(&err);
        assert_eq!(Some(ErrorCode::MailboxNotFound), code, "{err:#}");
        assert!(events.is_empty(), "got {events:?}");
        assert_eq!(None, opened);
    }

    /// The resolver's own connection keeps the server's refusal typed.
    #[test]
    fn a_resolve_refused_with_overquota_is_quota_exceeded() {
        let config = refuse("OVERQUOTA");
        let shutdown = Arc::new(AtomicBool::new(false));
        let mut resolver = Resolver::new(&config, "INBOX", &shutdown);

        let err = resolver.summary("1").unwrap_err();
        assert_eq!(
            Some(ErrorCode::QuotaExceeded),
            crate::error::code(&err),
            "{err:#}"
        );
    }
}
