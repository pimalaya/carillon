---
cairn: log
change: an-alarm-keyed-by-its-listed-id
landed: 2026-11-15
---

# An alarm keyed by its listed id

carillon's part of line W8 of the 2026-11-15 CLI alignment round landed, on io-webdav 0.10.0. Spec moved: daemon (modified Bodies are read only when an alarm hook wants them).

- io-webdav 0.10 reads a listed id percent-decoded when the decoding encodes back to it. The alarm book took those ids from `calendar-query`, but a poll's members were named by the raw last segment of their href, so on 0.10 a changed event whose href held an escape (`Daily%20standup.ics`) kept its former alarm and a removed one fired on. `refresh_alarms` now names each member through io-webdav's `id_from_location`.
- `an_alarm_follows_a_member_whose_href_holds_an_escape` (src/dav.rs) drives the listing watch, the query and the multiget over a scripted server: it fails with the raw segment, the alarm staying at 08:45, and passes with the fix.
- No hook id changes spelling: an item hook's `$id` is the href as served, which 0.10 does not read; an alarm hook's `$id` is the served href, or `join_path` of the decoded id, which rebuilds the served segment byte for byte. carillon stores nothing between runs, and `under` sends a configured collection id as before, so the decoded id a cardamum or calendula listing now prints (`My Book`) watches the collection the encoded one did.
