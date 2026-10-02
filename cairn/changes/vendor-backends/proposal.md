---
cairn: change
id: vendor-backends
status: landed
created: 2026-10-02
---

# Watch Microsoft Graph and Google accounts natively

## Why

A Microsoft 365 or Google account can only be watched over IMAP, CalDAV or CardDAV today, and the [wizard](../wizard/proposal.md) settled that on purpose: "carillon has no proprietary backend". That stopped being free. Microsoft is retiring basic auth on IMAP, Google gates IMAP behind app passwords or a restricted OAuth scope, and neither serves contacts or calendars over DAV in a shape worth watching (Google's CardDAV deltas come back empty, see the io-webdav notes). himalaya, cardamum and calendula already speak these APIs natively, and the libraries carillon would need are published: io-msgraph 0.4, io-gmail 0.4, io-gcal 0.1, io-gpeople 0.4, each with a change feed.

This change reverses the wizard's stance deliberately, for the reasons above.

## What

Four backends, each a block of its own, bearer-only, polled. None of these APIs pushes to a client without a public webhook, so `watch.poll.interval` is the one method each offers.

- `msgraph`, shaped like `jmap` after [jmap-domains](../jmap-domains/proposal.md): `msgraph.mailbox` (a mail folder), `msgraph.addressbook` (a contact folder) and `msgraph.calendar`, each optional, at least one required, one token and one connection for all three. Hooks: `on-message-*`, `on-flag-*` (`isRead` as `Seen`, a flagged follow-up as `Flagged`), `on-card-*`, `on-event-*`. Mail and contacts read Graph delta queries (`messages/delta`, `contacts/delta`) scoped to their folder. The calendar does not use `calendarView/delta`: its window is fixed at the first round, so an event leaving the window would read as a deletion. It lists the calendar's events with their `changeKey` and diffs that, the way a WebDAV poll diffs etags.
- `gmail`: `gmail.mailbox` is a label name, as neverest keys it, read through io-gmail's history poll. Hooks: `on-message-*` and `on-flag-*` (no `UNREAD` as `Seen`, `STARRED` as `Flagged`).
- `gcal`: `gcal.calendar`, matched by name or id (`primary` included), read through `events.list` with a sync token, series-level. Hooks: `on-event-*`. An expired token re-baselines silently.
- `gpeople`: `gpeople.addressbook` is a contact group, matched by name or resource name, `myContacts` being every contact the account owns, read through `connections.list` with a sync token and filtered by membership. Hooks: `on-card-*`.

Every block takes `tls`, `alpn`, `proxy` and `auth.token` like its sibling CLIs (`user-id` too for `msgraph` and `gmail`). An arrival resolves its envelope from the response the poll already reads, on `msgraph` and `gmail` as on JMAP.

The wizard routes a Google or Microsoft address to these backends rather than IMAP and DAV, and collects the token through the shared picker, ortie being the broker the sample and the prompts name. `-b/--backend` and `carillon check` learn the four names. Each is a cargo feature, on by default like the others.

## Not in this change

Push. Graph subscriptions and Gmail `users.watch` both need a public HTTPS endpoint or a Pub/Sub topic, which a local daemon does not have. Recurrence instances on Google and Graph calendars: an edit to one occurrence reports the series as changed.
