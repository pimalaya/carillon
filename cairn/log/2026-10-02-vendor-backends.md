---
cairn: log
change: vendor-backends
landed: 2026-10-02
---

# Watched Microsoft Graph and Google accounts natively

## What landed

Four backends, each behind its cargo feature and on by default, each bearer-only and polled: `msgraph`, `gmail`, `gcal` and `gpeople`, with their blocks, hook tables, `-b/--backend` names, `check` probes and wizard entries. The wizard's "no proprietary backend" stance is reversed on purpose.

`msgraph` takes a collection per domain the way `jmap` does, the two now sharing `DomainsHookConfig` (formerly `JmapHookConfig`) and the at-least-one and hook-to-domain checks; the domain-to-key mapping moved onto `WatchDomain::collection_name`. Mail and contacts read delta queries scoped to their folder, the default contact folder answering to `Contacts`; the calendar lists its events with their `changeKey` on every poll, `calendarView/delta` being refused for its fixed window. `gmail` runs `users.history.list` on the client rather than io-gmail's poll coroutine, whose cursor is private, so a lost connection resumes from the same cursor; `UNREAD` and `STARRED` are the shared `Seen` (inverted) and `Flagged`, and the envelope is read from the arrival's headers. `gcal` and `gpeople` read sync tokens; a cancelled event and a deleted or ungrouped contact are removals. `gcal` opens its own stream, io-gcal's connect taking no proxy.

The reconciliation the REST feeds share lives in a new `picture` module (`touch`, `remove`, `rebase` over a `Known` map of flags and versions), and the interruptible sleep in a new `poll` module, which also replaced the private copies in `dav`, `maildir` and `jmap`. `build.rs` gained the `network` and `api` cfgs, collapsing the feature lists the network-only and vendor-only items were gated on.

## What is still true

Every existing block loads and watches as before. The IMAP, JMAP and DAV entries are still offered for a Google or Microsoft address, after the vendor APIs rather than in their place, which is narrower than the proposal's "rather than IMAP and DAV".

## Note for verification

Build, clippy and the suite (68 tests) are green on every feature combination, each backend alone included. `check` and `watch` were run against the four live APIs with an invalid token: each reaches its API and reports the 401, and `watch` backs off and reopens. That run caught `gcal` passing `check` on `primary` with no request made, fixed by reading one event page. No hook was seen firing against a real tenant or Workspace account, so that task stays open. A Gmail cursor older than Gmail keeps resumes from now, what moved in between going unreported.
