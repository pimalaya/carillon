//! Local envelope resolver for the notify consumer.
//!
//! carillon-core's ring is content-free; it says the mailbox changed, not
//! what changed. The notify consumer is terminal — it cannot fetch on its
//! own — so, on a change, the daemon resolves the new messages here, on its
//! **own** IMAP connection (the consumer's own protocol client, never
//! carillon-core), and hands the notification their subject, sender, and
//! date. The `cmd` consumer is deliberately not enriched.
//!
//! This drives io-imap's coroutines directly with a blocking pump, exactly
//! as [`crate::driver`] drives the watch, but it is a short, one-shot
//! exchange: greet, authenticate, EXAMINE, `UID FETCH` the new range, done.

use std::{
    io::{Read, Write},
    num::NonZeroU32,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow, bail};
use carillon_core::credential::CarillonCredential;
use io_imap::{
    codec::fragmentizer::Fragmentizer,
    coroutine::{ImapCoroutine, ImapCoroutineState, ImapYield},
    rfc3501::{
        examine::{ImapMailboxExamine, ImapMailboxExamineOptions},
        fetch::{ImapMessageFetch, ImapMessageFetchOptions},
        greeting::{ImapGreetingGet, ImapGreetingGetOptions},
        login::{ImapLogin, ImapLoginOptions},
    },
    rfc7628::auth_oauthbearer::{ImapAuthOauthbearer, ImapAuthOauthbearerOptions},
    types::{
        core::NString,
        envelope::Address,
        fetch::{MacroOrMessageDataItemNames, MessageDataItem, MessageDataItemName},
        mailbox::Mailbox,
        sequence::SequenceSet,
    },
};
use secrecy::ExposeSecret;

use crate::net;

/// Only mailbox status and small envelopes are read, never a body, so the
/// parser never needs to grow large.
const MAX_MESSAGE_SIZE: u32 = 1 << 20;
/// Bounds each blocking read so a shutdown is noticed during a resolve.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// A newly-arrived message, resolved for a notification. Every field is
/// optional, since a server may omit any envelope part.
#[derive(Clone, Debug)]
pub struct CarillonImapMessage {
    /// The message UID.
    pub uid: u32,
    /// The first sender's personal name, when present.
    pub from_name: Option<String>,
    /// The first sender's `mailbox@host` address, when present.
    pub from_addr: Option<String>,
    /// The message subject, when present.
    pub subject: Option<String>,
    /// The message date, as the server rendered it, when present.
    pub date: Option<String>,
}

/// Opens a fresh connection, authenticates, and fetches the envelopes of the
/// messages that arrived since `since_uid` (the UID range
/// `[since_uid, uid_next)`). Returns the new messages in UID order and the
/// mailbox's current `uid_next`, so the caller can track the next range.
///
/// With no `since_uid` (the first change of a session) it resolves only the
/// single newest message, since there is no prior watermark to diff against.
pub fn resolve_new_messages(
    login: &str,
    host: &str,
    port: u16,
    credential: &CarillonCredential,
    mailbox: &Mailbox<'static>,
    since_uid: Option<u32>,
    shutdown: &Arc<AtomicBool>,
) -> Result<(Vec<CarillonImapMessage>, Option<u32>)> {
    let mut stream = net::connect(host, port, Some(READ_TIMEOUT))?;
    let mut frag = Fragmentizer::new(MAX_MESSAGE_SIZE);

    let greeting = ImapGreetingGet::new(ImapGreetingGetOptions {
        ensure_capabilities: false,
    });
    pump(&mut stream, &mut frag, greeting, shutdown)??;

    match credential {
        CarillonCredential::Password(password) => {
            let login = ImapLogin::new(
                login,
                password.expose_secret(),
                ImapLoginOptions {
                    ensure_capabilities: false,
                    auto_id: None,
                },
            )
            .map_err(|err| anyhow!("invalid IMAP credentials: {err}"))?;
            pump(&mut stream, &mut frag, login, shutdown)??;
        }
        CarillonCredential::Bearer(token) => {
            let oauth = ImapAuthOauthbearer::new(
                login,
                host,
                port,
                token.expose_secret(),
                ImapAuthOauthbearerOptions {
                    initial_request: true,
                    ensure_capabilities: false,
                    auto_id: None,
                },
            );
            pump(&mut stream, &mut frag, oauth, shutdown)??;
        }
    }

    let examine = ImapMailboxExamine::new(
        mailbox.clone(),
        ImapMailboxExamineOptions {
            parameters: Vec::new(),
        },
    );
    let data = pump(&mut stream, &mut frag, examine, shutdown)??;
    let uid_next = data.uid_next.map(|uid| uid.get());

    let messages = match new_range(since_uid, uid_next) {
        Some((lo, hi)) => {
            let lo = NonZeroU32::new(lo).ok_or_else(|| anyhow!("invalid low UID"))?;
            let hi = NonZeroU32::new(hi).ok_or_else(|| anyhow!("invalid high UID"))?;
            let sequence = SequenceSet::from(lo..=hi);
            let items = MacroOrMessageDataItemNames::MessageDataItemNames(vec![
                MessageDataItemName::Uid,
                MessageDataItemName::Envelope,
            ]);
            let fetch = ImapMessageFetch::new(
                sequence,
                items,
                ImapMessageFetchOptions {
                    uid: true,
                    modifiers: Vec::new(),
                },
            );
            let fetched = pump(&mut stream, &mut frag, fetch, shutdown)??;
            parse_messages(fetched)
        }
        None => Vec::new(),
    };

    Ok((messages, uid_next))
}

/// The inclusive UID range of newly-arrived messages, or `None` when nothing
/// new can be resolved. With a prior watermark it is `[since_uid, uid_next)`;
/// with none it is the single newest UID.
fn new_range(since_uid: Option<u32>, uid_next: Option<u32>) -> Option<(u32, u32)> {
    let uid_next = uid_next?;
    match since_uid {
        Some(since) if uid_next > since => Some((since, uid_next - 1)),
        Some(_) => None,
        None if uid_next > 1 => Some((uid_next - 1, uid_next - 1)),
        None => None,
    }
}

/// Folds a FETCH response into new-message summaries, in UID order.
fn parse_messages(
    fetched: std::collections::BTreeMap<
        NonZeroU32,
        io_imap::types::core::Vec1<MessageDataItem<'static>>,
    >,
) -> Vec<CarillonImapMessage> {
    let mut messages: Vec<CarillonImapMessage> = fetched
        .into_values()
        .filter_map(|items| {
            let mut uid = None;
            let mut envelope = None;
            for item in items {
                match item {
                    MessageDataItem::Uid(value) => uid = Some(value.get()),
                    MessageDataItem::Envelope(value) => envelope = Some(value),
                    _ => {}
                }
            }
            let envelope = envelope?;
            Some(CarillonImapMessage {
                uid: uid.unwrap_or(0),
                from_name: envelope.from.first().and_then(|from| nstring(&from.name)),
                from_addr: envelope.from.first().and_then(from_address),
                subject: nstring(&envelope.subject),
                date: nstring(&envelope.date),
            })
        })
        .collect();
    messages.sort_by_key(|message| message.uid);
    messages
}

/// Renders an address as `mailbox@host`, when both parts are present.
fn from_address(address: &Address<'_>) -> Option<String> {
    let mailbox = nstring(&address.mailbox)?;
    let host = nstring(&address.host)?;
    Some(format!("{mailbox}@{host}"))
}

/// Decodes an IMAP nstring into an owned `String`, lossily, or `None` for NIL.
fn nstring(value: &NString<'_>) -> Option<String> {
    value
        .0
        .as_ref()
        .map(|string| String::from_utf8_lossy(string.as_ref()).into_owned())
}

/// Drives one non-IDLE io-imap coroutine to completion over `stream`,
/// performing each read and write it asks for. Returns the coroutine's own
/// result (itself a `Result`), so callers use `??`.
fn pump<C>(
    stream: &mut pimalaya_stream::std::stream::StreamStd,
    frag: &mut Fragmentizer,
    mut coroutine: C,
    shutdown: &Arc<AtomicBool>,
) -> Result<C::Return>
where
    C: ImapCoroutine<Yield = ImapYield>,
{
    let mut buf = [0u8; 8 * 1024];
    let mut pending: Option<usize> = None;
    loop {
        let input = pending.take().map(|n| &buf[..n]);
        match coroutine.resume(frag, input) {
            ImapCoroutineState::Yielded(ImapYield::WantsRead) => loop {
                if shutdown.load(Ordering::SeqCst) {
                    bail!("shutdown while resolving");
                }
                match stream.read(&mut buf) {
                    Ok(0) => bail!("connection closed by peer"),
                    Ok(n) => {
                        pending = Some(n);
                        break;
                    }
                    Err(err) if net::is_timeout(&err) => continue,
                    Err(err) => return Err(err).context("read failed"),
                }
            },
            ImapCoroutineState::Yielded(ImapYield::WantsWrite(bytes)) => {
                stream.write_all(&bytes).context("write failed")?;
            }
            ImapCoroutineState::Complete(result) => return Ok(result),
        }
    }
}
