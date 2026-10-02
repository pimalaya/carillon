//! What the live suites share: tokens minted from the environment, and a
//! [`Watcher`], one `carillon watch` running in the background on a
//! throwaway collection, its hooks appending to a file the test reads.
//!
//! A test arms the watcher, makes a change through the protocol crate,
//! and waits for the hook it should fire. The protocol crates are tested
//! live on their own, so these prove what carillon adds: that a change
//! reaches the right hook through the backend's change feed.
//!
//! Google tokens are minted from a service account key with domain-wide
//! delegation, inline in `GOOGLE_SERVICE_ACCOUNT_KEY` or at the path
//! `GOOGLE_SERVICE_ACCOUNT_KEY_FILE`, acting as
//! `GOOGLE_SERVICE_ACCOUNT_SUBJECT` (`google@pimalaya.org` by default).
//! Microsoft tokens are app-only, minted from `MSGRAPH_TENANT_ID`,
//! `MSGRAPH_CLIENT_ID` and `MSGRAPH_CLIENT_SECRET`, acting on
//! `MSGRAPH_USER_ID` (`microsoft@pimalaya.onmicrosoft.com` by default).

#![allow(dead_code)]

use std::{
    borrow::Cow,
    cell::Cell,
    env, fs,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use io_oauth::{
    client::Oauth20ClientStd,
    rfc6749::client_credentials::*,
    rfc7523::{
        assertion::{Oauth20JwtBearerClaims, Oauth20JwtBearerKey},
        auth_grant::Oauth20JwtBearerGrantRequestParams,
    },
};
use pimalaya_stream::tls::Tls;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tempfile::TempDir;
use url::Url;

/// The Pimalaya Workspace user the service account acts as by default.
const GOOGLE_SUBJECT: &str = "google@pimalaya.org";

/// The Pimalaya test mailbox an app-only Graph token acts on by default.
const MSGRAPH_USER: &str = "microsoft@pimalaya.onmicrosoft.com";

/// The scope of an app-only Graph token: every permission the app holds.
const MSGRAPH_SCOPE: &str = "https://graph.microsoft.com/.default";

/// The account name every watcher's configuration declares.
const ACCOUNT: &str = "live";

/// The environment variable the configured token command prints.
const TOKEN_VAR: &str = "CARILLON_LIVE_TOKEN";

/// Seconds between two polls of a watcher, short to keep a run short.
const INTERVAL: u64 = 3;

/// How long the baseline, or a change reaching its hook, may take: the
/// providers acknowledge a write before every change feed shows it.
const SETTLE: Duration = Duration::from_secs(120);

/// A name no earlier run used, for every resource a run creates.
pub fn tag() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    format!("carillon-live-{millis}")
}

/// One `carillon watch` on the live account, killed when dropped.
pub struct Watcher {
    dir: TempDir,
    child: Child,
    /// How many hook lines the test has already consumed.
    seen: Cell<usize>,
}

impl Watcher {
    /// Starts a watch on an account holding one `backend` block, made of
    /// `block` and one hook per name in `hooks`, and waits until the
    /// collection is read: what changes from there is news.
    ///
    /// Each hook appends a line naming itself, the item id, and the
    /// subject and flag where its event carries them.
    pub fn start(backend: &str, block: &str, hooks: &[&str], token: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let events = dir.path().join("events");

        let hooks: String = hooks
            .iter()
            .map(|hook| {
                format!(
                    "{backend}.hook.{hook}.cmd = 'echo \"{hook} $id $subject $flag\" >> {}'\n",
                    events.display(),
                )
            })
            .collect();
        let config = format!(
            "[accounts.{ACCOUNT}]\n\
             {backend}.auth.token.command = \"printenv {TOKEN_VAR}\"\n\
             {backend}.watch.poll.interval = {INTERVAL}\n\
             {block}\n\
             {hooks}",
        );
        fs::write(dir.path().join("config.toml"), config).unwrap();
        fs::write(&events, "").unwrap();

        let child = Command::new(env!("CARGO_BIN_EXE_carillon"))
            .arg("-c")
            .arg(dir.path().join("config.toml"))
            .arg("--log-file")
            .arg(dir.path().join("log"))
            .args(["-a", ACCOUNT, "-b", backend, "watch"])
            .env("RUST_LOG", "carillon=debug")
            .env(TOKEN_VAR, token)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn carillon");

        let watcher = Self {
            dir,
            child,
            seen: Cell::new(0),
        };

        // NOTE: each backend logs `watching <backend> …` once it holds
        // its picture, the info line before it naming no backend first.
        let armed = format!("watching {backend} ");
        watcher.until("the watch reads the collection", || {
            watcher.log().contains(&armed)
        });

        watcher
    }

    /// Waits for the next line `hook` appends, and hands it back.
    ///
    /// Lines are consumed in order, so a test waiting for an arrival,
    /// then a flag, then a removal reads each after the one before.
    pub fn wait_for(&self, hook: &str, what: &str) -> String {
        let prefix = format!("{hook} ");
        let mut found = None;

        self.until(what, || {
            let events = fs::read_to_string(self.dir.path().join("events")).unwrap();
            let lines: Vec<&str> = events.lines().collect();
            let next = lines
                .iter()
                .enumerate()
                .skip(self.seen.get())
                .find(|(_, line)| line.starts_with(&prefix));

            match next {
                Some((index, line)) => {
                    self.seen.set(index + 1);
                    found = Some(line.to_string());
                    true
                }
                None => false,
            }
        });

        found.unwrap()
    }

    /// Lets a few polls run, so a change made just before is part of the
    /// watch's picture before the next one is made.
    pub fn settle(&self) {
        thread::sleep(Duration::from_secs(INTERVAL * 3));
    }

    /// Polls `check` until it holds, failing with the watch's own log.
    fn until(&self, what: &str, mut check: impl FnMut() -> bool) {
        let deadline = Instant::now() + SETTLE;

        while !check() {
            if Instant::now() > deadline {
                let events = fs::read_to_string(self.dir.path().join("events")).unwrap();
                panic!(
                    "timed out waiting until {what}\n--- hooks ---\n{events}\n--- log ---\n{}",
                    self.log(),
                );
            }

            thread::sleep(Duration::from_millis(500));
        }
    }

    /// The watch's log so far.
    fn log(&self) -> String {
        fs::read_to_string(self.dir.path().join("log")).unwrap_or_default()
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Runs `body`, then `cleanup` whichever way `body` went, and only then
/// re-raises a panic `body` may have raised, so a failed run leaves
/// nothing behind on the live account.
pub fn with_cleanup(body: impl FnOnce(), cleanup: impl FnOnce()) {
    let outcome = panic::catch_unwind(AssertUnwindSafe(body));

    if panic::catch_unwind(AssertUnwindSafe(cleanup)).is_err() {
        eprintln!("WARNING: cleanup failed, the live account may hold leftovers");
    }

    if let Err(payload) = outcome {
        panic::resume_unwind(payload);
    }
}

/// The subset of a service account key the JWT bearer grant needs.
#[derive(Deserialize)]
struct ServiceAccountKey {
    client_email: String,
    private_key: String,
    #[serde(default = "default_token_uri")]
    token_uri: String,
}

fn default_token_uri() -> String {
    String::from("https://oauth2.googleapis.com/token")
}

/// A Google access token for `scope`, traded for a JWT the service account
/// signs on behalf of the delegated subject (RFC 7523 section 2.1).
pub fn google_token(scope: &str) -> String {
    let key = match env::var("GOOGLE_SERVICE_ACCOUNT_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            let path = env::var("GOOGLE_SERVICE_ACCOUNT_KEY_FILE").expect(
                "set GOOGLE_SERVICE_ACCOUNT_KEY or GOOGLE_SERVICE_ACCOUNT_KEY_FILE to mint a token",
            );
            fs::read_to_string(PathBuf::from(path)).expect("read the service account key")
        }
    };
    let key: ServiceAccountKey = serde_json::from_str(&key).expect("parse the service account key");
    let subject =
        env::var("GOOGLE_SERVICE_ACCOUNT_SUBJECT").unwrap_or_else(|_| String::from(GOOGLE_SUBJECT));

    let signer = Oauth20JwtBearerKey::from_pkcs8_pem(&key.private_key)
        .expect("the service account key holds a PKCS#8 private key");
    let token_uri: Url = key.token_uri.parse().expect("the token URI is a URL");
    let mut client =
        Oauth20ClientStd::connect(token_uri, &Tls::default(), key.client_email.as_str())
            .expect("connect to the token endpoint");

    let claims = Oauth20JwtBearerClaims {
        iss: key.client_email.as_str().into(),
        sub: Some(subject.into()),
        scope: [Cow::from(scope.to_owned())].into_iter().collect(),
        ..Default::default()
    };
    let assertion = client
        .sign_jwt_bearer_assertion(&signer, claims, None, Duration::from_secs(600))
        .expect("sign the assertion");
    let params = Oauth20JwtBearerGrantRequestParams {
        assertion,
        scope: Default::default(),
    };

    match client
        .request_jwt_bearer_grant(params)
        .expect("request the token")
    {
        Ok(granted) => granted.access_token.expose_secret().to_owned(),
        Err(err) => panic!("the token endpoint refused the assertion: {err:?}"),
    }
}

/// The mailbox an app-only Graph token acts on.
pub fn msgraph_user() -> String {
    env::var("MSGRAPH_USER_ID").unwrap_or_else(|_| String::from(MSGRAPH_USER))
}

/// An app-only Graph token, traded for the app's client secret (RFC 6749
/// section 4.4).
pub fn msgraph_token() -> String {
    let var = |name: &str| {
        env::var(name).unwrap_or_else(|_| {
            panic!(
                "set MSGRAPH_TENANT_ID, MSGRAPH_CLIENT_ID and MSGRAPH_CLIENT_SECRET \
                 to mint a token ({name} is missing)"
            )
        })
    };
    let tenant = var("MSGRAPH_TENANT_ID");
    let client_id = var("MSGRAPH_CLIENT_ID");

    let token_uri: Url = format!("https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token")
        .parse()
        .expect("the token URI is a URL");
    let mut client = Oauth20ClientStd::connect(token_uri, &Tls::default(), client_id.as_str())
        .expect("connect to the token endpoint");
    client.client_secret = Some(SecretString::from(var("MSGRAPH_CLIENT_SECRET")));

    let params = Oauth20ClientCredentialsRequestParams {
        scope: [Cow::from(MSGRAPH_SCOPE)].into_iter().collect(),
    };

    match client
        .request_client_credentials(params)
        .expect("request the token")
    {
        Ok(granted) => granted.access_token.expose_secret().to_owned(),
        Err(err) => panic!("the token endpoint refused the client: {err:?}"),
    }
}

/// An RFC 5322 message to `to`, whose subject and `Message-ID` carry
/// `tag`.
pub fn message(tag: &str, to: &str) -> String {
    format!(
        "From: Carillon <{to}>\r\n\
         To: {to}\r\n\
         Subject: {tag}\r\n\
         Date: Thu, 01 Jan 2026 00:00:00 +0000\r\n\
         Message-ID: <{tag}@pimalaya.org>\r\n\
         MIME-Version: 1.0\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         \r\n\
         Sent by the carillon live tests.\r\n",
    )
}
