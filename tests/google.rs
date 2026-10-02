//! Live tests against Google: Gmail, Calendar and People, on the
//! Pimalaya Workspace user. Ignored by default.
//!
//! Each watches one throwaway label, calendar or contact group and checks
//! every hook a change to it should fire (see tests/common). Every
//! resource a run creates is named `carillon-live-<millis>` and deleted
//! however the run ends. Run with a service account key holding
//! domain-wide delegation for the three scopes below:
//!
//! ```sh
//! GOOGLE_SERVICE_ACCOUNT_KEY_FILE=key.json \
//! cargo test --test google -- --ignored --test-threads=1
//! ```

#![cfg(any(feature = "gmail", feature = "gcal", feature = "gpeople"))]

mod common;

use std::slice;

#[cfg(feature = "gcal")]
use io_gcal::v3::{
    client::{GcalClientStd, GcalClientStdConnectOptions},
    rest::{
        calendars::GcalCalendar,
        events::{GcalEvent, GcalEventDateTime},
    },
};
#[cfg(feature = "gmail")]
use io_gmail::v1::{
    client::{GmailClientStd, GmailClientStdConnectOptions},
    rest::{
        labels::GmailLabel,
        messages::{
            GmailInternalDateSource, GmailMessage, encode_raw, insert::GmailMessageInsert,
            list::GmailMessagesListParams,
        },
    },
};
#[cfg(feature = "gpeople")]
use io_gpeople::v1::{
    client::{GpeopleClientStd, GpeopleClientStdConnectOptions},
    rest::{
        contact_groups::GpeopleContactGroup,
        people::{GpeopleName, GpeoplePerson, GpeoplePersonField},
    },
};
#[cfg(feature = "gpeople")]
use pimalaya_stream::{proxy::Proxy, tls::Tls};

use crate::common::*;

#[cfg(feature = "gmail")]
const GMAIL_SCOPE: &str = "https://mail.google.com/";
#[cfg(feature = "gcal")]
const CALENDAR_SCOPE: &str = "https://www.googleapis.com/auth/calendar";
#[cfg(feature = "gpeople")]
const CONTACTS_SCOPE: &str = "https://www.googleapis.com/auth/contacts";

/// A message filed under the watched label arrives with its envelope, its
/// labels are reported as the shared flags, and losing the label leaves.
#[test]
#[ignore = "live: needs a Google service account key"]
#[cfg(feature = "gmail")]
fn a_gmail_label_fires_its_hooks() {
    let token = google_token(GMAIL_SCOPE);
    let connect = || {
        GmailClientStd::connect(&token, GmailClientStdConnectOptions::default())
            .expect("connect to Gmail")
    };
    let tag = tag();
    let label = connect()
        .label_create(&GmailLabel {
            name: tag.clone(),
            ..Default::default()
        })
        .expect("create the label")
        .response
        .id;

    with_cleanup(
        || {
            let watcher = Watcher::start(
                "gmail",
                &format!("gmail.mailbox = \"{tag}\""),
                &["on-message-added", "on-flag-added", "on-message-removed"],
                &token,
            );

            let mut client = connect();
            let to = client
                .profile_get()
                .expect("read the profile")
                .response
                .email_address;
            let inserted = GmailMessage {
                raw: Some(encode_raw(message(&tag, &to).as_bytes())),
                label_ids: vec![label.clone(), String::from("UNREAD")],
                ..Default::default()
            };
            let insert = GmailMessageInsert::new(
                &client.auth,
                &client.user_id,
                &inserted,
                Some(GmailInternalDateSource::DateHeader),
                false,
            )
            .expect("prepare the insertion");
            let message = client.run(insert).expect("insert the message").response.id;

            let line = watcher.wait_for("on-message-added", "the arrival fires");
            assert!(line.contains(&tag), "with its subject: {line}");

            client
                .message_modify(
                    &message,
                    &[String::from("STARRED")],
                    &[String::from("UNREAD")],
                )
                .expect("read and star the message");

            let first = watcher.wait_for("on-flag-added", "a flag fires");
            let second = watcher.wait_for("on-flag-added", "the other flag fires");
            let both = format!("{first}\n{second}");
            assert!(both.contains("Seen"), "read is Seen: {both}");
            assert!(both.contains("Flagged"), "starred is Flagged: {both}");

            client
                .message_modify(&message, &[], slice::from_ref(&label))
                .expect("take the label off");
            watcher.wait_for("on-message-removed", "losing the label fires");
        },
        || {
            // NOTE: an inserted message can be answered under an id that
            // is not its own, so the run's copies are found back by their
            // Message-ID.
            let query = format!("rfc822msgid:{tag}@pimalaya.org");
            let params = GmailMessagesListParams {
                q: Some(&query),
                include_spam_trash: true,
                ..Default::default()
            };
            let mut client = connect();

            if let Ok(found) = client.messages_list(&params) {
                for message in found.response.messages {
                    if let Err(err) = client.message_delete(&message.id) {
                        eprintln!("WARNING: leftover message {}: {err:?}", message.id);
                    }
                }
            }

            if let Err(err) = client.label_delete(&label) {
                eprintln!("WARNING: leftover label {tag}: {err:?}");
            }
        },
    );
}

/// An event created, edited and deleted in the watched calendar fires the
/// three event hooks.
#[test]
#[ignore = "live: needs a Google service account key"]
#[cfg(feature = "gcal")]
fn a_google_calendar_fires_its_hooks() {
    let token = google_token(CALENDAR_SCOPE);
    let connect = || {
        GcalClientStd::connect(&token, GcalClientStdConnectOptions::default())
            .expect("connect to Calendar")
    };
    let tag = tag();
    let calendar = connect()
        .calendar_insert(&GcalCalendar {
            summary: Some(tag.clone()),
            ..Default::default()
        })
        .expect("create the calendar")
        .response
        .id
        .expect("the calendar has an id");

    with_cleanup(
        || {
            let watcher = Watcher::start(
                "gcal",
                &format!("gcal.calendar = \"{calendar}\""),
                &["on-event-added", "on-event-changed", "on-event-removed"],
                &token,
            );

            let at = |hour: u32| {
                Some(GcalEventDateTime {
                    date_time: Some(format!("2028-11-10T{hour:02}:00:00Z")),
                    ..Default::default()
                })
            };

            let mut client = connect();
            let event = client
                .event_insert(
                    &calendar,
                    &GcalEvent {
                        summary: Some(tag.clone()),
                        start: at(9),
                        end: at(10),
                        ..Default::default()
                    },
                    &Default::default(),
                )
                .expect("create the event")
                .response
                .id
                .expect("the event has an id");
            watcher.wait_for("on-event-added", "the arrival fires");

            client
                .event_patch(
                    &calendar,
                    &event,
                    &GcalEvent {
                        summary: Some(format!("{tag} edited")),
                        ..Default::default()
                    },
                    &Default::default(),
                    None,
                )
                .expect("edit the event");
            watcher.wait_for("on-event-changed", "the edit fires");

            client
                .event_delete(&calendar, &event, None, None)
                .expect("delete the event");
            watcher.wait_for("on-event-removed", "the removal fires");
        },
        || {
            if let Err(err) = connect().calendar_delete(&calendar) {
                eprintln!("WARNING: leftover calendar {tag}: {err:?}");
            }
        },
    );
}

/// A contact joining the watched group, edited, then leaving it fires the
/// three card hooks.
#[test]
#[ignore = "live: needs a Google service account key"]
#[cfg(feature = "gpeople")]
fn a_google_contact_group_fires_its_hooks() {
    let token = google_token(CONTACTS_SCOPE);
    let connect = || {
        let options = GpeopleClientStdConnectOptions {
            tls: Tls::default(),
            proxy: Proxy::None,
        };
        GpeopleClientStd::connect(&token, options).expect("connect to People")
    };
    let tag = tag();
    let group = connect()
        .contact_group_create(
            &GpeopleContactGroup {
                name: Some(tag.clone()),
                ..Default::default()
            },
            &[],
        )
        .expect("create the group")
        .response
        .resource_name;
    let person = connect()
        .contact_create(
            &GpeoplePerson {
                names: vec![GpeopleName {
                    given_name: Some(tag.clone()),
                    ..Default::default()
                }],
                ..Default::default()
            },
            &[GpeoplePersonField::Names],
            &[],
        )
        .expect("create the contact")
        .response
        .resource_name;

    with_cleanup(
        || {
            let watcher = Watcher::start(
                "gpeople",
                &format!("gpeople.addressbook = \"{tag}\""),
                &["on-card-added", "on-card-changed", "on-card-removed"],
                &token,
            );

            let mut client = connect();
            client
                .contact_group_members_modify(&group, slice::from_ref(&person), &[])
                .expect("add the contact to the group");
            watcher.wait_for("on-card-added", "joining the group fires");

            // NOTE: an update must name the current etag, which joining
            // the group moved.
            let current = client
                .person_get(&person, &[GpeoplePersonField::Names], &[])
                .expect("read the contact")
                .response;
            let edited = GpeoplePerson {
                resource_name: person.clone(),
                etag: current.etag,
                names: vec![GpeopleName {
                    given_name: Some(tag.clone()),
                    family_name: Some(String::from("Edited")),
                    ..Default::default()
                }],
                ..Default::default()
            };
            client
                .contact_update(
                    &edited,
                    &[GpeoplePersonField::Names],
                    &[GpeoplePersonField::Names],
                    &[],
                )
                .expect("edit the contact");
            watcher.wait_for("on-card-changed", "the edit fires");

            client
                .contact_group_members_modify(&group, &[], slice::from_ref(&person))
                .expect("take the contact out of the group");
            watcher.wait_for("on-card-removed", "leaving the group fires");
        },
        || {
            let mut client = connect();

            if let Err(err) = client.contact_delete(&person) {
                eprintln!("WARNING: leftover contact {person}: {err:?}");
            }

            if let Err(err) = client.contact_group_delete(&group, false) {
                eprintln!("WARNING: leftover group {tag}: {err:?}");
            }
        },
    );
}
