---
cairn: log
change: jmap-reconnect-resolves-its-credential
landed: 2026-11-18
---

# A JMAP reconnect resolves its credential

Finding CRL-1 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4.5), confirmed on 2026-11-16 by a run against a loopback JMAP server answering 401 to an expired token. Spec moved: daemon (The daemon owns the connection lifecycle, a sentence and a scenario).

- `jmap::reconnect` resolved a fresh credential through `open`, then kept only its stream (`client.set_stream(fresh.stream)`), so the retried round presented the first token again, failed 401, and lost the session; the next session's baseline already held the change, which fired nothing. It now replaces the whole client (`*client = fresh`), as gmail.rs, gcal.rs, gpeople.rs and msgraph.rs already did. The session is read again with it, which costs one request per retried round.
- `jmap::subscribe` built its event source from the round client's session and `http_auth`, the token the session started with, while it opened a connection of its own with a fresh one. It now opens that connection first and builds the event source from its session and credential, and no longer takes the client.
- Tests in src/jmap.rs: `a_reconnect_presents_the_credential_resolved_again` and `a_subscription_presents_the_credential_resolved_again`, on a loopback JMAP server taking only the token a shared cell names, the token command reading a temp file switched from `T1` to `T2` after the session opened. Both fail on the HEAD code (e4db5c0, the 401 unwrapped) and pass after.
