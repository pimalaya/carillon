---
cairn: log
change: gmail-recipient-and-truncated-alarm-query
landed: 2026-11-18
---

# The Gmail recipient and a truncated alarm query

Finding CRL-10 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4.5), and the Gmail half of CRL-4 the coordinator asked for on 2026-11-18. Spec moved: daemon (Arrivals are resolved only when a hook wants them, two sentences; Bodies are read only when an alarm hook wants them, a sentence and a scenario).

- `gmail::summarize` took `To` up to its first comma (`to.split(',').next()`), so a quoted display name holding a comma gave `"Doe` as `$recipient`, and `party` split a mailbox on its last `<` by hand. `party` now reads the header through mime-rs's `tree::field::address::parse_addresses` (RFC 5322 §3.4, obsolete syntax and groups read liberally, encoded words in names decoded) and takes the first mailbox, as IMAP, JMAP and Graph take the first of their lists. The subject goes through `codec::rfc2047::decode_text`, as IMAP's does since 2026-11-18. Whether the Gmail API hands encoded words in its metadata headers was not checked: decoding text holding none is a no-op, so both readings are covered.
- mime-rs gains its `parser` feature (memchr, already in the tree) for `parse_addresses`, and the `gmail` feature now names `dep:mime-rs`; Cargo.lock adds memchr to mime-rs's dependency list alone, no crate entering the tree.
- `dav::read_alarms` warned on a truncated `calendar-query` and kept what it got, so the alarms of every member the server left out never fired until the session was lost. `query` now hands back `truncated`, and when one of the queries (or the unfiltered fallback) was truncated, the members of the watch's picture no query named are read through `calendar-multiget`, `MULTIGET_PAGE` (100) at a time, the book keeping those holding an alarm as before. The picture is complete at that point, a `sync-collection` baseline being drained when truncated; a server with no `sync-collection` whose PROPFIND listing was itself truncated still leaves out what neither named. `read_alarms` now takes the `Watched` rather than its collection, for the picture.
- Tests: src/gmail.rs `a_to_of_several_addresses_reads_as_its_first_and_names_decode` and src/dav.rs `a_truncated_alarm_query_reads_the_members_it_left_out` (the fake DAV server now answers a `calendar-query` with two bodies at most, then a 507 row) fail on the HEAD code (the subject and `To` read raw, two alarms of three) and pass after.
