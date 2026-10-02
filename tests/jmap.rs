//! Live tests against JMAP: a local Stalwart for the three domains, and
//! Fastmail for mail. Ignored by default.
//!
//! Each watches throwaway collections and checks every hook a change to
//! them should fire (see tests/common). Every change is made through
//! plain JMAP method calls, io-jmap writing no calendar event. Everything
//! a run creates is named `carillon-live-<millis>` and deleted however
//! the run ends.
//!
//! The Stalwart tests run against the instance tests/stalwart.sh
//! provisions on port 8082:
//!
//! ```sh
//! ./tests/stalwart.sh
//! cargo test --test jmap stalwart -- --ignored --test-threads=1
//! ```
//!
//! The Fastmail test takes an API token with the mail scope:
//!
//! ```sh
//! FASTMAIL_API_TOKEN=… cargo test --test jmap fastmail -- --ignored
//! ```

#![cfg(feature = "jmap")]

mod common;

use std::env;

use base64::{Engine, prelude::BASE64_STANDARD};
use io_jmap::{
    client::{JmapClientStd, JmapClientStdConnectOptions},
    rfc8620::request::JmapRequest,
};
use pimalaya_stream::proxy::Proxy;
use serde_json::{Value, json};
use url::Url;

use crate::common::*;

/// The session endpoint tests/stalwart.sh exposes.
const STALWART_SERVER: &str = "http://localhost:8082/jmap/session";
/// The user tests/stalwart.sh provisions.
const STALWART_USER: &str = "test@pimalaya.org";
/// Its password, which Stalwart's strength check accepts.
const STALWART_PASSWORD: &str = "P!malaya-test-2026";
/// Fastmail's session endpoint.
const FASTMAIL_SERVER: &str = "https://api.fastmail.com/jmap/session";

/// The capabilities a call of the tests declares, where the server
/// advertises them.
const USING: &[&str] = &[
    "urn:ietf:params:jmap:core",
    "urn:ietf:params:jmap:mail",
    "urn:ietf:params:jmap:contacts",
    "urn:ietf:params:jmap:calendars",
];

/// A JMAP session the test makes its changes through.
struct Jmap {
    client: JmapClientStd,
    account: String,
}

impl Jmap {
    /// Opens a session on `server` with the `Authorization` value `auth`.
    fn open(server: &str, auth: &str) -> Self {
        let url = Url::parse(server).unwrap();
        let opts = JmapClientStdConnectOptions {
            proxy: Proxy::None,
            ..Default::default()
        };
        let mut client =
            JmapClientStd::connect(&url, auth.to_string().into(), opts).expect("connect to JMAP");
        let account = client
            .session_get(&url)
            .expect("read the session")
            .primary_account_id_for("urn:ietf:params:jmap:mail");

        Self { client, account }
    }

    /// The address the session belongs to.
    fn username(&self) -> String {
        self.client.session().unwrap().username.clone()
    }

    /// The capabilities of [`USING`] the server advertises, a server
    /// refusing a request that declares one it lacks.
    fn using(&self) -> Vec<String> {
        let advertised = &self.client.session().unwrap().capabilities;

        USING
            .iter()
            .filter(|using| advertised.contains_key(**using))
            .map(|using| using.to_string())
            .collect()
    }

    /// Runs one `<Type>/set` and hands back its response.
    fn set(&mut self, method: &str, mut args: Value) -> Value {
        args["accountId"] = json!(self.account);

        let request = JmapRequest {
            using: self.using(),
            method_calls: vec![(method.to_string(), args, String::from("0"))],
            created_ids: None,
        };
        let response = self.client.send_raw(request).expect("send the call");
        let (name, response, _) = response.method_responses.into_iter().next().unwrap();
        assert_eq!(method, name, "{method} failed: {response}");

        for refused in ["notCreated", "notUpdated", "notDestroyed"] {
            let refused = &response[refused];
            assert!(
                refused.is_null() || refused.as_object().is_some_and(|o| o.is_empty()),
                "{method} refused: {refused}",
            );
        }

        response
    }

    /// Creates one object through `<Type>/set` and hands back its id.
    fn create(&mut self, method: &str, object: Value) -> String {
        let response = self.set(method, json!({ "create": { "new": object } }));
        response["created"]["new"]["id"]
            .as_str()
            .expect("the created object has an id")
            .to_string()
    }

    /// Patches one object through `<Type>/set`.
    fn update(&mut self, method: &str, id: &str, patch: Value) {
        self.set(method, json!({ "update": { id: patch } }));
    }

    /// Destroys one object through `<Type>/set`, with `extra` arguments.
    fn destroy(&mut self, method: &str, id: &str, extra: Value) {
        let mut args = json!({ "destroy": [id] });
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        self.set(method, args);
    }
}

/// The `Authorization` value of the Stalwart test user.
fn stalwart_auth() -> String {
    let credentials = format!("{STALWART_USER}:{STALWART_PASSWORD}");
    format!("Basic {}", BASE64_STANDARD.encode(credentials))
}

/// The `jmap` lines of a Stalwart account, its password read from the
/// environment.
fn stalwart_block(collections: &str) -> String {
    format!(
        "jmap.server = \"{STALWART_SERVER}\"\n\
         jmap.auth.basic.username = \"{STALWART_USER}\"\n\
         jmap.auth.basic.password.command = \"printenv CARILLON_LIVE_TOKEN\"\n\
         {collections}",
    )
}

/// A plain-text message from and to `to`, under `mailbox`, whose subject
/// is `tag`.
fn email(mailbox: &str, to: &str, tag: &str) -> Value {
    json!({
        "mailboxIds": { mailbox: true },
        "from": [{ "email": to }],
        "to": [{ "email": to }],
        "subject": tag,
        "bodyValues": { "1": { "value": "Sent by the carillon live tests." } },
        "textBody": [{ "partId": "1", "type": "text/plain" }],
    })
}

/// A message arrives with its envelope, two keywords are reported as the
/// shared flags, and its destruction leaves.
fn mail_fires_its_hooks(jmap: &mut Jmap, watcher: &Watcher, mailbox: &str, tag: &str) {
    let to = jmap.username();
    let message = jmap.create("Email/set", email(mailbox, &to, tag));

    let line = watcher.wait_for("on-message-added", "the arrival fires");
    assert!(line.contains(tag), "with its subject: {line}");

    let patch = json!({ "keywords/$seen": true, "keywords/$flagged": true });
    jmap.update("Email/set", &message, patch);

    let first = watcher.wait_for("on-flag-added", "a flag fires");
    let second = watcher.wait_for("on-flag-added", "the other flag fires");
    let both = format!("{first}\n{second}");
    assert!(both.contains("Seen"), "$seen is Seen: {both}");
    assert!(both.contains("Flagged"), "$flagged is Flagged: {both}");

    jmap.destroy("Email/set", &message, json!({}));
    watcher.wait_for("on-message-removed", "the removal fires");
}

/// The mail hooks the mail tests arm.
const MAIL_HOOKS: &[&str] = &["on-message-added", "on-flag-added", "on-message-removed"];

/// One account watching a mailbox, an addressbook and a calendar at once,
/// polled over one connection, fires the hooks of all three domains.
#[test]
#[ignore = "live: needs tests/stalwart.sh"]
fn stalwart_mail_contacts_and_calendar_fire_their_hooks() {
    let tag = tag();
    let mut jmap = Jmap::open(STALWART_SERVER, &stalwart_auth());
    let mailbox = jmap.create("Mailbox/set", json!({ "name": tag }));
    let book = jmap.create("AddressBook/set", json!({ "name": tag }));
    let calendar = jmap.create("Calendar/set", json!({ "name": tag }));

    with_cleanup(
        || {
            let collections = format!(
                "jmap.mailbox = \"{tag}\"\n\
                 jmap.addressbook = \"{book}\"\n\
                 jmap.calendar = \"{calendar}\"\n\
                 jmap.watch.poll.interval = 3",
            );
            let hooks = [
                MAIL_HOOKS,
                &["on-card-added", "on-card-changed", "on-card-removed"],
                &["on-event-added", "on-event-changed", "on-event-removed"],
            ]
            .concat();
            let watcher = Watcher::start_with(
                "jmap",
                &stalwart_block(&collections),
                &hooks,
                STALWART_PASSWORD,
                3,
            );

            mail_fires_its_hooks(&mut jmap, &watcher, &mailbox, &tag);

            let card = json!({
                "@type": "Card",
                "version": "1.0",
                "addressBookIds": { &book: true },
                "name": { "full": tag },
            });
            let card = jmap.create("ContactCard/set", card);
            watcher.wait_for("on-card-added", "the card arrival fires");

            let edited = json!({ "name": { "full": format!("{tag} edited") } });
            jmap.update("ContactCard/set", &card, edited);
            watcher.wait_for("on-card-changed", "the card edit fires");

            jmap.destroy("ContactCard/set", &card, json!({}));
            watcher.wait_for("on-card-removed", "the card removal fires");

            let event = json!({
                "@type": "Event",
                "calendarIds": { &calendar: true },
                "title": tag,
                "start": "2028-11-10T09:00:00",
                "timeZone": "Etc/UTC",
                "duration": "PT1H",
            });
            let event = jmap.create("CalendarEvent/set", event);
            watcher.wait_for("on-event-added", "the event arrival fires");

            let edited = json!({ "title": format!("{tag} edited") });
            jmap.update("CalendarEvent/set", &event, edited);
            watcher.wait_for("on-event-changed", "the event edit fires");

            jmap.destroy("CalendarEvent/set", &event, json!({}));
            watcher.wait_for("on-event-removed", "the event removal fires");
        },
        || {
            let mut jmap = Jmap::open(STALWART_SERVER, &stalwart_auth());
            let remove_mail = json!({ "onDestroyRemoveEmails": true });
            jmap.destroy("Mailbox/set", &mailbox, remove_mail);
            let remove_cards = json!({ "onDestroyRemoveContents": true });
            jmap.destroy("AddressBook/set", &book, remove_cards);
            let remove_events = json!({ "onDestroyRemoveEvents": true });
            jmap.destroy("Calendar/set", &calendar, remove_events);
        },
    );
}

/// A mailbox watched over the held event stream, the default method,
/// fires its hooks as the server pushes.
#[test]
#[ignore = "live: needs tests/stalwart.sh"]
fn stalwart_mail_is_pushed() {
    let tag = tag();
    let mut jmap = Jmap::open(STALWART_SERVER, &stalwart_auth());
    let mailbox = jmap.create("Mailbox/set", json!({ "name": tag }));

    with_cleanup(
        || {
            let block = stalwart_block(&format!("jmap.mailbox = \"{tag}\""));
            let watcher = Watcher::start_with("jmap", &block, MAIL_HOOKS, STALWART_PASSWORD, 1);
            mail_fires_its_hooks(&mut jmap, &watcher, &mailbox, &tag);
        },
        || {
            let mut jmap = Jmap::open(STALWART_SERVER, &stalwart_auth());
            let remove_mail = json!({ "onDestroyRemoveEmails": true });
            jmap.destroy("Mailbox/set", &mailbox, remove_mail);
        },
    );
}

/// A Fastmail mailbox watched over the held event stream fires its hooks
/// as Fastmail pushes.
#[test]
#[ignore = "live: needs a Fastmail API token"]
fn fastmail_mail_is_pushed() {
    let token = env::var("FASTMAIL_API_TOKEN").expect("set FASTMAIL_API_TOKEN");
    let auth = format!("Bearer {token}");
    let tag = tag();
    let mut jmap = Jmap::open(FASTMAIL_SERVER, &auth);
    let mailbox = jmap.create("Mailbox/set", json!({ "name": tag }));

    with_cleanup(
        || {
            let block = format!(
                "jmap.server = \"{FASTMAIL_SERVER}\"\n\
                 jmap.auth.bearer.token.command = \"printenv CARILLON_LIVE_TOKEN\"\n\
                 jmap.mailbox = \"{tag}\"",
            );
            let watcher = Watcher::start_with("jmap", &block, MAIL_HOOKS, &token, 1);
            mail_fires_its_hooks(&mut jmap, &watcher, &mailbox, &tag);
        },
        || {
            let mut jmap = Jmap::open(FASTMAIL_SERVER, &auth);
            let remove_mail = json!({ "onDestroyRemoveEmails": true });
            jmap.destroy("Mailbox/set", &mailbox, remove_mail);
        },
    );
}
