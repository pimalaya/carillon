//! Live tests against Microsoft Graph: mail, contacts and calendars, on
//! the Pimalaya test mailbox. Ignored by default.
//!
//! Each watches one throwaway folder or calendar and checks every hook a
//! change to it should fire (see tests/common). Every resource a run
//! creates is named `carillon-live-<millis>` and deleted however the run
//! ends. Run with the app registration's client secret (the app needs
//! `Mail.ReadWrite`, `Contacts.ReadWrite` and `Calendars.ReadWrite`):
//!
//! ```sh
//! MSGRAPH_TENANT_ID=… MSGRAPH_CLIENT_ID=… MSGRAPH_CLIENT_SECRET=… \
//! cargo test --test msgraph -- --ignored --test-threads=1
//! ```

#![cfg(feature = "msgraph")]

mod common;

use io_msgraph::v1::{
    client::{MsgraphClientStd, MsgraphClientStdConnectOptions},
    field::MsgraphField,
    rest::users::{
        calendars::MsgraphCalendar,
        contact_folders::MsgraphContactFolder,
        contacts::MsgraphContact,
        events::{MsgraphDateTimeTimeZone, MsgraphEvent},
        mail_folders::MsgraphMailFolder,
        messages::{MsgraphFlagStatus, MsgraphFollowupFlag, MsgraphMessage},
    },
};

use crate::common::*;

/// Opens a Graph client on the test mailbox.
fn connect(token: &str) -> MsgraphClientStd {
    let options = MsgraphClientStdConnectOptions {
        user_id: msgraph_user(),
        ..Default::default()
    };
    MsgraphClientStd::connect(token, options).expect("connect to Graph")
}

/// The `msgraph` block watching `key` = `id` on the test mailbox.
fn block(key: &str, id: &str) -> String {
    format!(
        "msgraph.user-id = \"{}\"\nmsgraph.{key} = \"{id}\"",
        msgraph_user()
    )
}

/// A message filed in the watched folder arrives with its envelope, an
/// edit of the draft is reported, its flags are reported under the shared
/// names, and its deletion leaves.
#[test]
#[ignore = "live: needs the app registration's client secret"]
fn a_graph_mail_folder_fires_its_hooks() {
    let token = msgraph_token();
    let tag = tag();
    let folder = connect(&token)
        .mail_folder_create(&MsgraphMailFolder {
            display_name: tag.clone(),
            ..Default::default()
        })
        .expect("create the folder")
        .response
        .id;

    with_cleanup(
        || {
            let watcher = Watcher::start(
                "msgraph",
                &block("mailbox", &folder),
                &[
                    "on-message-added",
                    "on-message-changed",
                    "on-flag-added",
                    "on-message-removed",
                ],
                &token,
            );

            let mut client = connect(&token);
            let message = client
                .message_create_mime(Some(&folder), message(&tag, &msgraph_user()).as_bytes())
                .expect("file the message")
                .response
                .id;

            let line = watcher.wait_for("on-message-added", "the arrival fires");
            assert!(line.contains(&tag), "with its subject: {line}");

            let edit = MsgraphMessage {
                subject: Some(format!("{tag} edited")),
                ..Default::default()
            };
            client
                .message_update(&message, &edit)
                .expect("edit the draft");
            watcher.wait_for("on-message-changed", "the edit fires");

            // NOTE: a message filed through MIME is a draft, which Graph
            // may already count as read, so it is made unread first and
            // given a poll to be seen that way.
            let unread = MsgraphMessage {
                is_read: Some(false),
                ..Default::default()
            };
            client
                .message_update(&message, &unread)
                .expect("mark the message unread");
            watcher.settle();

            let patch = MsgraphMessage {
                is_read: Some(true),
                flag: Some(MsgraphFollowupFlag {
                    flag_status: Some(MsgraphFlagStatus::Flagged),
                }),
                ..Default::default()
            };
            client
                .message_update(&message, &patch)
                .expect("read and flag the message");

            let first = watcher.wait_for("on-flag-added", "a flag fires");
            let second = watcher.wait_for("on-flag-added", "the other flag fires");
            let both = format!("{first}\n{second}");
            assert!(both.contains("Seen"), "read is Seen: {both}");
            assert!(both.contains("Flagged"), "a follow-up is Flagged: {both}");

            client.message_delete(&message).expect("delete the message");
            watcher.wait_for("on-message-removed", "the removal fires");
        },
        || {
            if let Err(err) = connect(&token).mail_folder_delete(&folder) {
                eprintln!("WARNING: leftover folder {tag}: {err:?}");
            }
        },
    );
}

/// A contact created, edited and deleted in the watched folder fires the
/// three card hooks.
#[test]
#[ignore = "live: needs the app registration's client secret"]
fn a_graph_contact_folder_fires_its_hooks() {
    let token = msgraph_token();
    let tag = tag();
    let folder = connect(&token)
        .contact_folder_create(&MsgraphContactFolder {
            display_name: tag.clone(),
            ..Default::default()
        })
        .expect("create the contact folder")
        .response
        .id;

    with_cleanup(
        || {
            let watcher = Watcher::start(
                "msgraph",
                &block("addressbook", &folder),
                &["on-card-added", "on-card-changed", "on-card-removed"],
                &token,
            );

            let mut client = connect(&token);
            let contact = client
                .contact_create(
                    Some(&folder),
                    &MsgraphContact {
                        given_name: MsgraphField::Set(tag.clone()),
                        ..Default::default()
                    },
                )
                .expect("create the contact")
                .response
                .id;
            watcher.wait_for("on-card-added", "the arrival fires");

            client
                .contact_update(
                    &contact,
                    &MsgraphContact {
                        surname: MsgraphField::Set(String::from("Edited")),
                        ..Default::default()
                    },
                )
                .expect("edit the contact");
            watcher.wait_for("on-card-changed", "the edit fires");

            client.contact_delete(&contact).expect("delete the contact");
            watcher.wait_for("on-card-removed", "the removal fires");
        },
        || {
            if let Err(err) = connect(&token).contact_folder_delete(&folder) {
                eprintln!("WARNING: leftover contact folder {tag}: {err:?}");
            }
        },
    );
}

/// An event created, edited and deleted in the watched calendar fires the
/// three event hooks, read through a listing rather than a windowed delta.
#[test]
#[ignore = "live: needs the app registration's client secret"]
fn a_graph_calendar_fires_its_hooks() {
    let token = msgraph_token();
    let tag = tag();
    let calendar = connect(&token)
        .calendar_create(&MsgraphCalendar {
            name: MsgraphField::Set(tag.clone()),
            ..Default::default()
        })
        .expect("create the calendar")
        .response
        .id;

    with_cleanup(
        || {
            let watcher = Watcher::start(
                "msgraph",
                &block("calendar", &calendar),
                &["on-event-added", "on-event-changed", "on-event-removed"],
                &token,
            );

            // NOTE: far ahead, which a windowed delta would not see.
            let at = |hour: u32| {
                MsgraphField::Set(MsgraphDateTimeTimeZone {
                    date_time: format!("2028-11-10T{hour:02}:00:00"),
                    time_zone: Some(String::from("UTC")),
                })
            };

            let mut client = connect(&token);
            let event = client
                .event_create(
                    Some(&calendar),
                    &MsgraphEvent {
                        subject: MsgraphField::Set(tag.clone()),
                        start: at(9),
                        end: at(10),
                        ..Default::default()
                    },
                )
                .expect("create the event")
                .response
                .id;
            watcher.wait_for("on-event-added", "the arrival fires");

            client
                .event_update(
                    &event,
                    &MsgraphEvent {
                        subject: MsgraphField::Set(format!("{tag} edited")),
                        ..Default::default()
                    },
                )
                .expect("edit the event");
            watcher.wait_for("on-event-changed", "the edit fires");

            client.event_delete(&event).expect("delete the event");
            watcher.wait_for("on-event-removed", "the removal fires");
        },
        || {
            if let Err(err) = connect(&token).calendar_delete(&calendar) {
                eprintln!("WARNING: leftover calendar {tag}: {err:?}");
            }
        },
    );
}
