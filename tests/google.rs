//! Live tests against Google: Gmail, Calendar and People, on the
//! Pimalaya Workspace user. Ignored by default.
//!
//! Each watches one label, calendar or contact group and checks every hook
//! a change to it should fire (see tests/common). Every label, message,
//! contact group, contact and event a run creates is named
//! `carillon-live-<millis>` and deleted however the run ends. The calendar
//! is long-lived, `pimalaya-live-carillon-hooks`, created when absent and
//! never deleted, since Google caps the calendars a user may create a day.
//! Run with a service account key holding domain-wide delegation for the
//! scopes below:
//!
//! ```sh
//! GOOGLE_SERVICE_ACCOUNT_KEY_FILE=key.json \
//! cargo test --test google -- --ignored --test-threads=1
//! ```

#![cfg(any(feature = "gmail", feature = "gcal", feature = "gpeople"))]

mod common;

#[cfg(any(feature = "gmail", feature = "gpeople"))]
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
const GMAIL_SCOPES: &[&str] = &["https://mail.google.com/"];
/// The events and the calendar list the gcal source reads, plus the
/// calendar the test creates when absent. The delegation
/// refuses the full `calendar` scope, and one refused scope fails the
/// whole token request.
#[cfg(feature = "gcal")]
const CALENDAR_SCOPES: &[&str] = &[
    "https://www.googleapis.com/auth/calendar.events",
    "https://www.googleapis.com/auth/calendar.calendarlist.readonly",
    "https://www.googleapis.com/auth/calendar.calendars",
];
#[cfg(feature = "gpeople")]
const CONTACTS_SCOPES: &[&str] = &["https://www.googleapis.com/auth/contacts"];

/// A message filed under the watched label arrives with its envelope, its
/// labels are reported as the shared flags, and losing the label leaves.
#[test]
#[ignore = "live: needs a Google service account key"]
#[cfg(feature = "gmail")]
fn a_gmail_label_fires_its_hooks() {
    let token = google_token(GMAIL_SCOPES);
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
                &["on-message-add", "on-flag-add", "on-message-remove"],
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

            let line = watcher.wait_for("on-message-add", "the arrival fires");
            assert!(line.contains(&tag), "with its subject: {line}");

            client
                .message_modify(
                    &message,
                    &[String::from("STARRED")],
                    &[String::from("UNREAD")],
                )
                .expect("read and star the message");

            let first = watcher.wait_for("on-flag-add", "a flag fires");
            let second = watcher.wait_for("on-flag-add", "the other flag fires");
            let both = format!("{first}\n{second}");
            assert!(both.contains("Seen"), "read is Seen: {both}");
            assert!(both.contains("Flagged"), "starred is Flagged: {both}");

            client
                .message_modify(&message, &[], slice::from_ref(&label))
                .expect("take the label off");
            watcher.wait_for("on-message-remove", "losing the label fires");
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
    let token = google_token(CALENDAR_SCOPES);
    let connect = || {
        GcalClientStd::connect(&token, GcalClientStdConnectOptions::default())
            .expect("connect to Calendar")
    };
    let tag = tag();
    let calendar = google_calendar(&mut connect(), "hooks");
    clear_google_calendar(&mut connect(), &calendar);

    with_cleanup(
        || {
            let watcher = Watcher::start(
                "gcal",
                &format!("gcal.calendar = \"{calendar}\""),
                &["on-event-add", "on-event-change", "on-event-remove"],
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
            watcher.wait_for("on-event-add", "the arrival fires");

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
            watcher.wait_for("on-event-change", "the edit fires");

            client
                .event_delete(&calendar, &event, None, None)
                .expect("delete the event");
            watcher.wait_for("on-event-remove", "the removal fires");
        },
        || clear_google_calendar(&mut connect(), &calendar),
    );
}

/// The id of the long-lived calendar `pimalaya-live-carillon-<purpose>`,
/// created when the calendar list holds none, never deleted.
#[cfg(feature = "gcal")]
fn google_calendar(client: &mut GcalClientStd, purpose: &str) -> String {
    use io_gcal::v3::rest::calendar_list::list::GcalCalendarListListParams;

    let summary = format!("pimalaya-live-carillon-{purpose}");

    let mut page = None;
    loop {
        let params = GcalCalendarListListParams {
            page_token: page.as_deref(),
            ..Default::default()
        };
        let list = client
            .calendar_list_list(&params)
            .expect("list the calendars")
            .response;
        let found = list
            .items
            .into_iter()
            .find(|entry| entry.summary.as_deref() == Some(summary.as_str()));
        if let Some(id) = found.and_then(|entry| entry.id) {
            return id;
        }
        match list.next_page_token {
            Some(token) => page = Some(token),
            None => break,
        }
    }

    client
        .calendar_insert(&GcalCalendar {
            summary: Some(summary),
            ..Default::default()
        })
        .expect("create the calendar")
        .response
        .id
        .expect("the calendar has an id")
}

/// Deletes every event of the test's own `calendar`, the leftovers of an
/// earlier run at the start, the run's own at the end.
#[cfg(feature = "gcal")]
fn clear_google_calendar(client: &mut GcalClientStd, calendar: &str) {
    use io_gcal::v3::rest::events::{GcalSendUpdates, list::GcalEventsListParams};

    let mut ids = Vec::new();
    let mut page = None;
    loop {
        let params = GcalEventsListParams {
            page_token: page.as_deref(),
            ..Default::default()
        };
        let events = client
            .events_list(calendar, &params)
            .expect("list the calendar's events")
            .response;
        ids.extend(events.items.into_iter().filter_map(|event| event.id));
        match events.next_page_token {
            Some(token) => page = Some(token),
            None => break,
        }
    }

    for id in ids {
        let none = Some(GcalSendUpdates::None);
        if let Err(err) = client.event_delete(calendar, &id, none, None) {
            eprintln!("WARNING: leftover event {id} in {calendar}: {err:?}");
        }
    }
}

/// A contact joining the watched group, edited, then leaving it fires the
/// three card hooks.
#[test]
#[ignore = "live: needs a Google service account key"]
#[cfg(feature = "gpeople")]
fn a_google_contact_group_fires_its_hooks() {
    let token = google_token(CONTACTS_SCOPES);
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
                &["on-card-add", "on-card-change", "on-card-remove"],
                &token,
            );

            let mut client = connect();
            client
                .contact_group_members_modify(&group, slice::from_ref(&person), &[])
                .expect("add the contact to the group");
            watcher.wait_for("on-card-add", "joining the group fires");

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
            watcher.wait_for("on-card-change", "the edit fires");

            client
                .contact_group_members_modify(&group, &[], slice::from_ref(&person))
                .expect("take the contact out of the group");
            watcher.wait_for("on-card-remove", "leaving the group fires");
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
