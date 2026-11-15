---
cairn: change
id: an-alarm-keyed-by-its-listed-id
status: landed
created: 2026-11-15
---

# An alarm keyed by its listed id

## Why

Line W8 of the CLI alignment round of 2026-11-15 (pimalaya/.github cairn/changes/cli-alignment-2026-11-15), carillon's part, after io-webdav 0.10.0 (line W7, io-webdav cairn/changes/archive/a-listed-id-is-decoded). io-webdav 0.10 reads an id from a path segment percent-decoded when the decoding encodes back to it: a member listed at `/cal/alice/work/Daily%20standup.ics` carries the id `Daily standup.ics` in a `calendar-query` and a `calendar-multiget`. carillon's alarm book is keyed by those ids (src/dav.rs `read_alarms`), but `refresh_alarms` named a member a poll reported by the raw last segment of its href (`resource`, `Daily%20standup.ics`). On 0.10, a changed member was multigot but matched no entry, so its key was forgotten under the raw name, which the book does not hold: the alarm kept its former trigger and the new one was never read. A removed member's alarm was kept and fired on.

## What

- io-webdav 0.10.0 (Cargo.lock, the requirement raised to "0.10.0"). carillon calls none of the APIs 0.10 breaks (`CaldavCalendarUpdate`, `update_calendar`, `WebdavResponseEntry::id`).
- `resource` reads a member's id through io-webdav's `rfc4918::id_from_location`, so the poll, the query and the multiget name a member alike.
- A scripted-server test, `an_alarm_follows_a_member_whose_href_holds_an_escape`: a calendar member at `Daily%20standup.ics` whose alarm moves with a change and goes with a removal.

## Ids checked

- **Hooks.** An item hook's `$id` is the href the `sync-collection` report or the `PROPFIND` listing serves, which io-webdav 0.10 does not read as an id: unchanged. An alarm hook's `$id` is `join_path(collection, id)` for a member the baseline query read and the served href for one a poll moved; 0.10 decodes only a segment its encoder gives back byte for byte, so `join_path` rebuilds the very segment served and the spelling is unchanged too (the test asserts both). No CHANGELOG line about a changed spelling, and no MIGRATION.md entry.
- **State.** carillon stores nothing between runs: the watch's picture is keyed by href, the alarm book is in memory.
- **Collections.** `under` sends a configured collection id through `collection_path`, which 0.10 leaves as it was: `carddav.addressbook = "My Book"`, the id a cardamum listing now prints, and `"My%20Book"`, the one it printed before, both read `<home>/My%20Book/`.

## Scope

The alarm `$id` of a member is a path rebuilt from the collection for one the baseline read and the href as served for one a poll moved; they differ only on a server serving absolute hrefs. That predates 0.10 and is left as it is.
