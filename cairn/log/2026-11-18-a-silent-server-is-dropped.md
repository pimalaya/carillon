---
cairn: log
change: a-silent-server-is-dropped
landed: 2026-11-18
---

# A silent server is dropped

Finding CRL-7 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4.5). Spec moved: daemon (Ctrl+C is prompt on every path, a sentence and a scenario).

- carillon's own read pumps, `dav::pump`, the discovery pump `dav::pump_redirect` and `imap::pump` (the envelope resolver's connection), took the one-second read deadline as a wakeup only (`is_timeout => continue`, no count): a request on a connection gone dead after a resume or a network change waited until TCP gave up, where the doc of `dav::open` said a poll against a silent server ends at the next deadline. Each pump now notes when the coroutine asked to read and fails with "Server sent nothing for 60 seconds" once `SILENCE` (60 s, 2 s under `cfg(test)`) has passed with no byte. The DAV round is then run again on a fresh connection, discovery falls back as on any failure, and the resolver reconnects once, as they already did on a read error. The `dav::open` and `imap::pump` docs say so.
- 60 seconds and not the deadline itself: a server may take seconds before the first byte of a large `calendar-query`, and a false positive costs a reconnect and a request run twice.
- Left out: the IDLE connection of an IMAP watch is io-imap's watcher, bounded by `imap.watch.idle.timeout`, and the JMAP event stream (`jmap::subscribe`) still waits on its own reads with no limit; a dead stream there holds a push watch until TCP gives up.
- Tests: src/dav.rs `a_request_the_server_never_answers_fails` (a server reading every request and answering none) and src/imap.rs `a_resolve_the_server_never_answers_fails` (`refuse(None)` leaving EXAMINE unanswered), each run on a thread and awaited for five `SILENCE`, time out on the HEAD code (d0f6df0) and pass after.
