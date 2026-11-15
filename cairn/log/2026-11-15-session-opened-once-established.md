---
cairn: log
change: watch-streams-events
landed: 2026-11-15
---

# A session is reported open once established

The open point watch-streams-events left: `session-opened` was printed when a session started, before its connection was proven, so a refused server printed `session-opened` then `session-lost`. Spec moved: `daemon` (A watch streams what it observes: `session-opened` once a session is established; scenario "A session that cannot open" added).

- Every backend watch (src/imap.rs, src/jmap.rs, src/maildir.rs, src/dav.rs, src/msgraph.rs, src/gmail.rs, src/gcal.rs, src/gpeople.rs, src/pimdir.rs) takes an `on_open` callback, called once the session is connected, authenticated and its collections read (the baseline taken, CalDAV alarms read), right before the first wait; src/watch.rs sends `session-opened` from it. A session failing before prints `session-lost` alone, and every reopened session prints `session-opened` again once established.
- IMAP is established once authenticated and its watch started: io-imap's `watch_mailbox` examines the mailbox on its worker and reports no readiness, so a refused EXAMINE still prints `session-opened` then `session-lost`. Closing that needs a readiness signal from io-imap.
- src/dav.rs: `watch` takes a `DavTarget` (collection and kind) instead of the two arguments, keeping it under clippy's argument limit; src/imap.rs: the private `watch` takes a `Wait` (`Idle` or `Poll`) for the same reason.
- Tests: tests/watch.rs `a_session_that_cannot_open_is_lost_and_never_opened` (a Maildir mailbox that does not exist: `session-lost`, never `session-opened`); src/imap.rs `a_refused_authentication_never_opens_the_session` (a local server answering `AUTHENTICATE` with `NO [AUTHENTICATIONFAILED]`), and `a_watch_reopened_after_a_lost_connection_reports_what_moved_meanwhile` now checks the session opens before it reports; src/pimdir.rs `a_watch_of_a_store_not_created_is_never_reported_open`. The first two failed with the old order restored (the event sent before the backend ran, `on_open` called before the IMAP login), the order then put back.
