---
cairn: change
id: jmap-domains
status: active
created: 2026-08-22
---

# Let one JMAP account watch its contacts and its calendar, not only its mail

## Why

The JMAP backend watches `Email` and nothing else. Its EventSource subscribes to one type, its rounds call `Email/changes` and `Email/get`, and `JmapHookConfig` declares four mail hooks, so `jmap.hook.on-card-added` is refused when the file is read. A Fastmail account watching mail over JMAP has to watch its addressbook and its calendar over CardDAV and CalDAV, against the same server, with a second and a third credential block.

That is backwards for the one protocol that does not need it. JMAP is the clean case for the domain split [hook-under-backend](../hook-under-backend/proposal.md) left open: `Email/changes`, `ContactCard/changes` and `CalendarEvent/changes` are separate method calls, so a change knows its domain for free. CalDAV had to be asked what its collection holds and, for a mixed calendar, read a member's content type to tell a VEVENT from a VTODO; JMAP is told, by which method answered.

The protocol crate is ready. io-jmap covers contacts (RFC 9610: `address_book_get`, `address_book_changes`, `contact_card_get`, `contact_card_query`, `contact_card_changes`) and calendars (draft-ietf-jmap-calendars-27: `calendar_get`, `calendar_changes`, `calendar_event_get`, `calendar_event_query`, `calendar_event_changes`). Nothing new has to be written below carillon.

And there is a payoff no other backend can offer: one session, one connection, one event stream covering all three domains. An account watching mail, contacts and a calendar over DAV holds three connections and three polls; over JMAP it can hold one stream that names which of the three moved.

## What

The `jmap` block takes a collection per domain it watches, each optional and at least one required: `jmap.mailbox` as today, plus `jmap.addressbook` and `jmap.calendar`. The hook table gains the domains' events: `on-card-added`, `on-card-removed` and `on-card-changed` beside the mail four, and `on-event-added`, `on-event-removed` and `on-event-changed`.

There is no `on-task-*`. JMAP calendars carry `CalendarEvent` and the draft has no task type, which is a protocol fact rather than a gap, and it is why CalDAV's `on-task-*` has no JMAP twin.

A hook naming a domain the account does not configure is refused when the file is read, the way a hook a backend cannot fire already is. It cannot be a serde refusal, since what is allowed depends on a sibling key rather than on the table's own shape, so it joins the template-variable check in `AccountConfig::validate`, naming the hook and the collection key it would need.

The EventSource subscribes to the types the account configured, and a round asks each configured domain what moved. The collection a hook templates against follows the event's domain rather than the backend: `$mailbox` for a message, `$addressbook` for a card, `$calendar` for an event, which makes `HookCollection` a per-event answer where it is currently a per-backend one.

## The invariant this bends, deliberately

[collection-under-backend](../collection-under-backend/proposal.md) settled that an account watches one collection, and that watching a second is a second account, "which is also how it gets its own hooks". A JMAP account watching three domains breaks the letter of that.

It keeps the reason. The rule exists so each watched thing has hooks of its own, and it still does: the domains do not share an event name, so `on-card-added` and `on-message-added` are as separate as two accounts would have made them. What is shared is the connection, the credential and the session, which is exactly what is worth sharing and what a second account would waste.

The alternatives were weighed and are worse. Splitting JMAP into `jmap`, `jmap-contacts` and `jmap-calendar` blocks would keep the invariant literally and match how DAV split, at the cost of three copies of one server and credential, three sessions and three event streams against a server that offers one; it throws away the only thing JMAP does better here. Requiring exactly one of the three keys per account would keep both the invariant and the strict serde refusal, and costs the same three connections for no gain over the split. Neither is worth the letter of a rule whose purpose survives.

## Not in this change

Discovery of which domains an account actually holds. The JMAP session lists its capabilities per account (`urn:ietf:params:jmap:mail`, `:contacts`, `:calendars`), so configuring a domain the server does not serve could be refused when the session is read rather than when the first round fails. That is a startup check like CalDAV's `supported-calendar-component-set`, and it wants its own change once there is something to check against.
