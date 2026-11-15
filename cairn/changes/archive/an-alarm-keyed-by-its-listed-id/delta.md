---
cairn: delta
change: an-alarm-keyed-by-its-listed-id
---

# Delta

The requirement is in cairn/spec/daemon.md: a member is keyed by the id io-webdav reads from its href, and a scenario names a member whose href holds an escape.

## ADDED Requirements

None.

## MODIFIED Requirements

### Requirement: Bodies are read only when an alarm hook wants them
A CalDAV watch SHALL read member bodies only when its account configures `on-alarm-trigger`. It SHALL then read, once the baseline is taken, the members holding a `VALARM` through one `calendar-query` (RFC 4791 7.8) per component, a `VEVENT` and a `VTODO`, the comp-filters of one query all having to match (RFC 4791 9.7.1); a server refusing the filter SHALL be asked for every member instead. After that it SHALL read the members a poll reports added or changed through `calendar-multiget` (RFC 4791 7.9). A member SHALL be keyed by the id io-webdav's `id_from_location` reads from its href, its last segment percent-decoded when the decoding encodes back to it, which is the id the query and the multiget give it, so a member whose href holds an escape is one member whichever report named it. A body that is not iCalendar SHALL be logged and skipped; a report that fails SHALL end the session, the next one reading every alarm again.

#### Scenario: An account with no alarm hook
- **GIVEN** a CalDAV account configuring `on-event-change` alone
- **WHEN** an event is edited
- **THEN** the hook fires, and no event body is read

#### Scenario: A member whose href holds an escape
- **GIVEN** a watch with an alarm hook, holding an event at `/cal/alice/work/Daily%20standup.ics` whose alarm fires at 08:45
- **WHEN** the event is moved an hour later on the server and the watch polls, then it is removed and the watch polls
- **THEN** the alarm fires at 09:45 and not at 08:45, then not at all

## REMOVED Requirements

None.
