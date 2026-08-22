---
cairn: delta
change: dav-backend
---

## ADDED Requirements

### Requirement: A WebDAV collection is watchable
The daemon SHALL watch a WebDAV collection, which covers CalDAV and CardDAV alike, by polling an RFC 6578 `sync-collection` report. It SHALL request `getetag` and nothing else, so a poll never carries a contact or an event; it SHALL keep an href to etag picture of the collection, so that a member it has never seen reads as an arrival and a known member whose etag moved reads as an edit. A truncated report SHALL be drained immediately rather than at the next interval, and a sync token the server rejects SHALL cause a re-enumeration, which reports nothing because a re-baseline is not news.

#### Scenario: A contact is edited
- **GIVEN** a watch on a CardDAV addressbook
- **WHEN** an existing contact is modified and its etag changes
- **THEN** `on-item-changed` fires with the member's href, and nothing is fetched of the contact itself

#### Scenario: The server forgets its history
- **GIVEN** a watch whose stored sync token is older than the server keeps
- **WHEN** the next report is refused
- **THEN** the collection is enumerated again, no event is fired for what was already there, and the watch continues from the fresh token

## MODIFIED Requirements

### Requirement: One change vocabulary across backends
Every backend SHALL report changes in one vocabulary: an item added, an item removed, an item changed, flags added, flags removed. Flags SHALL be reported under one set of names whatever the backend spells them as, so that a hook filter written once fires against IMAP `\Seen`, JMAP `$seen` and the Maildir `S` letter alike. A backend SHALL report only the events its protocol can express, which is a property of the protocol rather than a gap: mail is immutable, so nothing mail reports an edit, and WebDAV has no flags. The hooks SHALL be named after items rather than messages, and the message-shaped names SHALL keep working as aliases.

#### Scenario: A message is marked read on each mail backend
- **GIVEN** three accounts watching the same mailbox over IMAP, JMAP and Maildir
- **WHEN** a message is marked read on each
- **THEN** all three fire `on-flags-added` with the flag named `Seen`

#### Scenario: A configuration written before the rename
- **GIVEN** an account configuring `hooks.on-message-added`
- **WHEN** the daemon loads it
- **THEN** it is read as `on-item-added` and fires exactly as it did

## REMOVED Requirements
