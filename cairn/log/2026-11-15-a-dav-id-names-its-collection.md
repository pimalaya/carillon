---
cairn: log
change: a-dav-id-names-its-collection
landed: 2026-11-15
---

# A DAV id names its collection

carillon's part of line W2 of the 2026-11-15 CLI alignment round landed, on io-webdav 0.9.1. Spec moved: daemon (modified An account watches one collection, one way, A failure carries a stable code).

- A DAV collection id that is empty, `/` alone, or holds an empty, `.` or `..` segment is refused by `watch` and `account check` before a connection is opened, coded `calendar-not-found` or `addressbook-not-found`, through io-webdav's `check_collection_id`; `caldav.calendar = "work/x/.."` used to watch `work`, `".."` the home set's parent.
- The collection path is composed by io-webdav's `collection_path`: a relative id percent-encoded segment by segment, every path ending with a slash.
- No code reads `address-data` or `calendar-data` trimmed: the alarm bodies come from io-webdav's `CaldavItemEntry::data`, whole since 0.9.1, which ical-rs reads with its indentation.
- The refused id is not coded `usage-invalid`: it comes from the configuration, and the `account check` row and the `session-lost` event carry carillon's vocabulary only.
