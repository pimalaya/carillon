---
cairn: change
id: a-dav-id-names-its-collection
status: landed
created: 2026-11-15
---

# A DAV id names its collection

## Why

Line W2 of the CLI alignment round of 2026-11-15 (pimalaya/.github cairn/changes/cli-alignment-2026-11-15), after io-webdav 0.9.1 (lines W1, W3, W4). carillon composes the path of its DAV collection itself (src/dav.rs `under`), as io-webdav did before 0.9.1: an id was joined as given, so `caldav.calendar = ".."` addressed the home set's parent, `""` the home set itself, `"/"` the server root, and `work/x/..` read the calendar `work`, the url crate resolving the dot segments when the request was built. carillon only reads, so nothing was written, but the watch and `account check` reported on another resource than the one configured. A relative id holding a byte a path segment cannot hold raw was sent as given: `\` read as a separator, `|`, `[` and `]` left raw. carillon drives io-webdav's coroutines itself, so the client's own refusal of 0.9.1 never runs for it.

## What

- io-webdav 0.9.1 (Cargo.lock, the requirement raised to "0.9.1").
- `watch` and `account check` refuse a collection id io-webdav's `rfc4918::check_collection_id` refuses (empty, `/` alone, an empty, `.` or `..` segment, percent-encoded or `\`-separated forms included) before a connection is opened, coded `calendar-not-found` or `addressbook-not-found`. Not `usage-invalid`: the id comes from the configuration, not the command line, carillon's `usage-invalid` is pimalaya-cli's refusal of the grammar (exit 2, nothing ran), and the failure reaches an `account check` row or a `session-lost` event, both typed by carillon's vocabulary; an id naming no collection is the collection missing, which the caller fixes the same way. The watch retries it as any missing collection, opening nothing each time.
- `under` composes every collection path through io-webdav's `rfc4918::collection_path`: a relative id percent-encoded segment by segment, an escape already there kept, an absolute one as given, each ending with a slash.
- `calendar-data`: carillon reads only io-webdav's `CaldavItemEntry::data` (the alarm bodies), never trimming it; 0.9.1 hands it back whole, the indentation of a pretty-printed report included, which ical-rs reads. carillon reads no `address-data`.
