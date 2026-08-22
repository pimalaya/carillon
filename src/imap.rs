//! IMAP watching as an I/O-free coroutine.
//!
//! The watch does no I/O itself. [`CarillonImapWatch`] is a state machine
//! that composes io-imap's own coroutines (greeting, authentication,
//! EXAMINE, IDLE) and, on each [`resume`](CarillonImapWatch::resume),
//! returns what the driver must do next: read, write, mint a ring for a
//! changed state, or stop. [`crate::driver`] owns the socket, the
//! timeouts, and turning a [`CarillonImapWatchProgress::Changed`] into a
//! [`CarillonEvent`](crate::event::CarillonEvent). Only IDLE is required;
//! the state token is content-free.
//!
//! Staying I/O-free is what keeps a second driver possible: a server would
//! pump this same coroutine over an async socket, with no watcher to write
//! twice.
//!
//! Every response byte is parsed by imap-codec through io-imap's
//! coroutines, so the hostile-input surface is theirs and is fuzzed there.
//! What this module owns is the parser bound it picks
//! ([`MAX_MESSAGE_SIZE`]) and the phase machine around the reads.
//!
//! Because the ring is content-free, this is not io-imap's QRESYNC delta
//! watcher and keeps no shadow of the mailbox: it greets, authenticates,
//! EXAMINEs for the initial state, then holds IDLE and, on each wake,
//! re-EXAMINEs and surfaces a change when the state token advanced.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use io_imap::{
    codec::fragmentizer::Fragmentizer,
    coroutine::{ImapCoroutine, ImapCoroutineState, ImapYield},
    rfc2177::idle::{ImapIdle, ImapIdleOptions, ImapIdleYield},
    rfc3501::{
        examine::{ExamineData, ImapMailboxExamine, ImapMailboxExamineOptions},
        greeting::{ImapGreetingGet, ImapGreetingGetOptions},
        login::{ImapLogin, ImapLoginOptions},
    },
    rfc7628::auth_oauthbearer::{ImapAuthOauthbearer, ImapAuthOauthbearerOptions},
    types::{command::SelectParameter, mailbox::Mailbox, response::Capability},
};
use secrecy::ExposeSecret;

use crate::{
    backend::CarillonImapBackend,
    credential::CarillonCredential,
    error::{CarillonWatchError, CarillonWatchResult},
};

/// Safety bound on the parser buffer. Only mailbox status is read, never a
/// body, so it never needs to grow large; the fragmentizer grows lazily.
const MAX_MESSAGE_SIZE: u32 = 1 << 20;

/// What the driver must do to make progress, returned by each
/// [`CarillonImapWatch::resume`].
pub enum CarillonImapWatchProgress {
    /// Read from the socket and pass the bytes to the next `resume`.
    WantsRead,
    /// Write these bytes to the socket, then `resume` again with no input.
    WantsWrite(Vec<u8>),
    /// The mailbox state advanced to this token. Mint and dispatch a
    /// [`CarillonEvent`](crate::event::CarillonEvent) from it, then `resume` again with no input.
    Changed(String),
    /// The watch finished: shutdown was requested, or an error occurred.
    Done(CarillonWatchResult<()>),
}

/// An I/O-free IMAP watch.
///
/// Greets, authenticates (`LOGIN` or SASL `OAUTHBEARER`), reads the initial
/// mailbox state by a read-only EXAMINE, then holds IDLE and, on each wake,
/// re-EXAMINEs and surfaces a [`CarillonImapWatchProgress::Changed`] when
/// the state token advanced. With CONDSTORE the token is
/// `UIDVALIDITY:HIGHESTMODSEQ` (advancing on any change); otherwise
/// `UIDVALIDITY:UIDNEXT` (new mail only).
pub struct CarillonImapWatch {
    imap: CarillonImapBackend,
    credential: CarillonCredential,
    mailbox: Mailbox<'static>,
    fragmentizer: Fragmentizer,
    shutdown: Arc<AtomicBool>,
    idle_done: Arc<AtomicBool>,
    condstore: bool,
    state: Option<String>,
    phase: Phase,
}

/// The current step of the watch conversation.
enum Phase {
    Greeting(ImapGreetingGet),
    Auth(Auth),
    ExamineSeed(ImapMailboxExamine),
    Idle(ImapIdle),
    ExamineDiff(ImapMailboxExamine),
    Failed(String),
    Ended,
}

/// The authentication sub-coroutine, one per mechanism.
enum Auth {
    Login(ImapLogin),
    Oauth(ImapAuthOauthbearer),
}

/// The outcome of advancing the authentication phase.
enum AuthStep {
    /// The auth coroutine needs I/O; hand this back to the driver.
    Io(CarillonImapWatchProgress),
    /// Authentication finished; the loop advances to EXAMINE.
    Continue,
    /// Authentication ended the watch (missing IDLE, or an error).
    Done(CarillonImapWatchProgress),
}

impl CarillonImapWatch {
    /// Creates a watch over an as-yet-unopened connection. `shutdown`, when
    /// set (typically from a signal handler on another thread), winds the
    /// IDLE down cleanly and completes the watch.
    pub fn new(
        imap: CarillonImapBackend,
        credential: CarillonCredential,
        shutdown: Arc<AtomicBool>,
    ) -> Self {
        let (mailbox, phase) = match Mailbox::try_from(imap.mailbox.clone()) {
            Ok(mailbox) => (
                mailbox,
                Phase::Greeting(ImapGreetingGet::new(ImapGreetingGetOptions {
                    ensure_capabilities: true,
                })),
            ),
            Err(_) => (
                Mailbox::Inbox,
                Phase::Failed(format!("invalid mailbox name: {}", imap.mailbox)),
            ),
        };
        Self {
            imap,
            credential,
            mailbox,
            fragmentizer: Fragmentizer::new(MAX_MESSAGE_SIZE),
            shutdown,
            idle_done: Arc::new(AtomicBool::new(false)),
            condstore: false,
            state: None,
            phase,
        }
    }

    /// Advances the watch, returning the next action for the driver. After
    /// a [`WantsRead`](CarillonImapWatchProgress::WantsRead), pass the bytes
    /// just read as `input`; otherwise pass `None`.
    pub fn resume(&mut self, input: Option<&[u8]>) -> CarillonImapWatchProgress {
        use CarillonImapWatchProgress as Progress;

        let mut input = input;
        loop {
            match core::mem::replace(&mut self.phase, Phase::Ended) {
                Phase::Greeting(mut greeting) => {
                    match greeting.resume(&mut self.fragmentizer, input.take()) {
                        ImapCoroutineState::Yielded(ImapYield::WantsRead) => {
                            self.phase = Phase::Greeting(greeting);
                            return Progress::WantsRead;
                        }
                        ImapCoroutineState::Yielded(ImapYield::WantsWrite(bytes)) => {
                            self.phase = Phase::Greeting(greeting);
                            return Progress::WantsWrite(bytes);
                        }
                        ImapCoroutineState::Complete(Err(err)) => return done_err("greeting", err),
                        ImapCoroutineState::Complete(Ok(_)) => match self.build_auth() {
                            Ok(auth) => self.phase = Phase::Auth(auth),
                            Err(err) => return Progress::Done(Err(err)),
                        },
                    }
                }
                Phase::Auth(auth) => match self.resume_auth(auth, input.take()) {
                    AuthStep::Io(progress) => return progress,
                    AuthStep::Continue => {}
                    AuthStep::Done(progress) => return progress,
                },
                Phase::ExamineSeed(mut examine) => {
                    match examine.resume(&mut self.fragmentizer, input.take()) {
                        ImapCoroutineState::Yielded(ImapYield::WantsRead) => {
                            self.phase = Phase::ExamineSeed(examine);
                            return Progress::WantsRead;
                        }
                        ImapCoroutineState::Yielded(ImapYield::WantsWrite(bytes)) => {
                            self.phase = Phase::ExamineSeed(examine);
                            return Progress::WantsWrite(bytes);
                        }
                        ImapCoroutineState::Complete(Err(err)) => return done_err("examine", err),
                        ImapCoroutineState::Complete(Ok(data)) => {
                            self.state = Some(state_token(&data, self.condstore));
                            self.phase = Phase::Idle(self.build_idle());
                        }
                    }
                }
                Phase::Idle(mut idle) => {
                    if self.shutdown.load(Ordering::SeqCst) {
                        self.idle_done.store(true, Ordering::SeqCst);
                    }
                    match idle.resume(&mut self.fragmentizer, input.take()) {
                        ImapCoroutineState::Yielded(ImapIdleYield::Event(_)) => {
                            self.idle_done.store(true, Ordering::SeqCst);
                            self.phase = Phase::Idle(idle);
                        }
                        ImapCoroutineState::Yielded(ImapIdleYield::WantsRead) => {
                            self.phase = Phase::Idle(idle);
                            return Progress::WantsRead;
                        }
                        ImapCoroutineState::Yielded(ImapIdleYield::WantsWrite(bytes)) => {
                            self.phase = Phase::Idle(idle);
                            return Progress::WantsWrite(bytes);
                        }
                        ImapCoroutineState::Complete(Err(err)) => return done_err("idle", err),
                        ImapCoroutineState::Complete(Ok(())) => {
                            if self.shutdown.load(Ordering::SeqCst) {
                                return Progress::Done(Ok(()));
                            }
                            self.phase = Phase::ExamineDiff(self.build_examine());
                        }
                    }
                }
                Phase::ExamineDiff(mut examine) => {
                    match examine.resume(&mut self.fragmentizer, input.take()) {
                        ImapCoroutineState::Yielded(ImapYield::WantsRead) => {
                            self.phase = Phase::ExamineDiff(examine);
                            return Progress::WantsRead;
                        }
                        ImapCoroutineState::Yielded(ImapYield::WantsWrite(bytes)) => {
                            self.phase = Phase::ExamineDiff(examine);
                            return Progress::WantsWrite(bytes);
                        }
                        ImapCoroutineState::Complete(Err(err)) => return done_err("examine", err),
                        ImapCoroutineState::Complete(Ok(data)) => {
                            let next = state_token(&data, self.condstore);
                            let changed = self.state.as_deref() != Some(next.as_str());
                            self.state = Some(next.clone());
                            self.phase = Phase::Idle(self.build_idle());
                            if changed {
                                return Progress::Changed(next);
                            }
                        }
                    }
                }
                Phase::Failed(message) => {
                    return Progress::Done(Err(CarillonWatchError::Imap(message)));
                }
                Phase::Ended => return Progress::Done(Ok(())),
            }
        }
    }

    /// Advances the authentication phase, putting the sub-coroutine back on
    /// a yield and advancing to EXAMINE on success.
    fn resume_auth(&mut self, auth: Auth, input: Option<&[u8]>) -> AuthStep {
        use CarillonImapWatchProgress as Progress;
        match auth {
            Auth::Login(mut login) => match login.resume(&mut self.fragmentizer, input) {
                ImapCoroutineState::Yielded(ImapYield::WantsRead) => {
                    self.phase = Phase::Auth(Auth::Login(login));
                    AuthStep::Io(Progress::WantsRead)
                }
                ImapCoroutineState::Yielded(ImapYield::WantsWrite(bytes)) => {
                    self.phase = Phase::Auth(Auth::Login(login));
                    AuthStep::Io(Progress::WantsWrite(bytes))
                }
                ImapCoroutineState::Complete(Err(err)) => AuthStep::Done(done_err("login", err)),
                ImapCoroutineState::Complete(Ok(capabilities)) => self.after_auth(capabilities),
            },
            Auth::Oauth(mut oauth) => match oauth.resume(&mut self.fragmentizer, input) {
                ImapCoroutineState::Yielded(ImapYield::WantsRead) => {
                    self.phase = Phase::Auth(Auth::Oauth(oauth));
                    AuthStep::Io(Progress::WantsRead)
                }
                ImapCoroutineState::Yielded(ImapYield::WantsWrite(bytes)) => {
                    self.phase = Phase::Auth(Auth::Oauth(oauth));
                    AuthStep::Io(Progress::WantsWrite(bytes))
                }
                ImapCoroutineState::Complete(Err(err)) => {
                    AuthStep::Done(done_err("oauthbearer", err))
                }
                ImapCoroutineState::Complete(Ok(capabilities)) => self.after_auth(capabilities),
            },
        }
    }

    /// Post-authentication: require IDLE, record CONDSTORE support, and move
    /// to the seeding EXAMINE.
    fn after_auth(&mut self, capabilities: Vec<Capability<'static>>) -> AuthStep {
        if !capabilities.contains(&Capability::Idle) {
            return AuthStep::Done(CarillonImapWatchProgress::Done(Err(
                CarillonWatchError::Imap("server does not advertise IDLE; cannot watch".into()),
            )));
        }
        self.condstore = capabilities.contains(&Capability::CondStore)
            || capabilities.contains(&Capability::QResync);
        self.phase = Phase::ExamineSeed(self.build_examine());
        AuthStep::Continue
    }

    /// Builds the authentication sub-coroutine for the resolved credential.
    fn build_auth(&self) -> Result<Auth, CarillonWatchError> {
        match &self.credential {
            CarillonCredential::Password(password) => {
                let login = ImapLogin::new(
                    &self.imap.login,
                    password.expose_secret(),
                    ImapLoginOptions {
                        ensure_capabilities: true,
                        auto_id: None,
                    },
                )
                .map_err(|err| {
                    CarillonWatchError::Imap(format!("invalid IMAP credentials: {err}"))
                })?;
                Ok(Auth::Login(login))
            }
            CarillonCredential::Bearer(token) => {
                let oauth = ImapAuthOauthbearer::new(
                    &self.imap.login,
                    &self.imap.host,
                    self.imap.port,
                    token.expose_secret(),
                    ImapAuthOauthbearerOptions {
                        initial_request: true,
                        ensure_capabilities: true,
                        auto_id: None,
                    },
                );
                Ok(Auth::Oauth(oauth))
            }
        }
    }

    /// Builds a read-only EXAMINE, requesting CONDSTORE when the server
    /// advertised it so the state token can track HIGHESTMODSEQ.
    fn build_examine(&self) -> ImapMailboxExamine {
        let parameters = if self.condstore {
            vec![SelectParameter::CondStore]
        } else {
            Vec::new()
        };
        ImapMailboxExamine::new(
            self.mailbox.clone(),
            ImapMailboxExamineOptions { parameters },
        )
    }

    /// Builds a fresh IDLE with a new wind-down flag stored on `self`.
    fn build_idle(&mut self) -> ImapIdle {
        let done = Arc::new(AtomicBool::new(false));
        self.idle_done = done.clone();
        ImapIdle::new(done, ImapIdleOptions::default())
    }
}

/// Folds an EXAMINE response into the opaque state token. With `condstore`
/// it is `UIDVALIDITY:HIGHESTMODSEQ` (advancing on any change); otherwise
/// `UIDVALIDITY:UIDNEXT` (advancing on new mail only).
fn state_token(data: &ExamineData, condstore: bool) -> String {
    let validity = data.uid_validity.map(|uid| uid.get()).unwrap_or(0);
    let uid_next = || data.uid_next.map(|uid| u64::from(uid.get()));
    let seq = if condstore {
        data.highest_mod_seq.or_else(uid_next).unwrap_or(0)
    } else {
        uid_next().unwrap_or(0)
    };
    format!("{validity}:{seq}")
}

/// Ends the watch with a contextual IMAP error.
fn done_err(context: &str, err: impl core::fmt::Display) -> CarillonImapWatchProgress {
    CarillonImapWatchProgress::Done(Err(CarillonWatchError::Imap(format!("{context}: {err}"))))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, atomic::AtomicBool};

    use secrecy::SecretString;

    use super::{CarillonImapWatch, CarillonImapWatchProgress, MAX_MESSAGE_SIZE};
    use crate::{backend::CarillonImapBackend, credential::CarillonCredential};

    /// Drives `resume` up to `cap` steps, feeding `input` the first time
    /// the parser asks to read and empty slices (EOF-ish) thereafter.
    /// Returns whether the watch reached `Done`.
    fn drive(watch: &mut CarillonImapWatch, input: &[u8], cap: usize) -> bool {
        let mut fed = false;
        let mut next: Option<Vec<u8>> = None;

        for _ in 0..cap {
            match watch.resume(next.take().as_deref()) {
                CarillonImapWatchProgress::WantsRead => {
                    next = Some(if fed { Vec::new() } else { input.to_vec() });
                    fed = true;
                }
                CarillonImapWatchProgress::WantsWrite(_) => next = None,
                CarillonImapWatchProgress::Changed(_) => next = None,
                CarillonImapWatchProgress::Done(_) => return true,
            }
        }

        false
    }

    /// The parser bound is this crate's to choose, so it is this crate's
    /// to test. Announcing a literal one byte past it must end the watch:
    /// a server naming a size must not be able to make the daemon buffer
    /// it. Response parsing itself is imap-codec's, and fuzzed there.
    #[test]
    fn an_oversized_literal_ends_the_watch() {
        let announce = format!("* OK {{{}}}\r\n", u64::from(MAX_MESSAGE_SIZE) + 1);
        let mut watch = CarillonImapWatch::new(
            CarillonImapBackend {
                host: "imap.example.org".into(),
                port: 993,
                login: "user@example.org".into(),
                mailbox: "INBOX".into(),
            },
            CarillonCredential::Password(SecretString::from("password".to_owned())),
            Arc::new(AtomicBool::new(false)),
        );

        assert!(
            drive(&mut watch, announce.as_bytes(), 256),
            "an over-bound literal must terminate the watch, not stall"
        );
    }
}
