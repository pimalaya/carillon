---
cairn: change
id: collection-under-backend
status: landed
created: 2026-08-22
---

# Put the collection under its backend too, and name it for what it holds

## Why

`collection` is the last account-level key describing something only a backend knows. It is required whatever the account declares, so an account with an `imap` block has to spell its mailbox under a word chosen to also cover a calendar, and an account with a `caldav` block has to spell a calendar path under a word chosen to also cover a mailbox. Neither reads like the thing it names, and the account is the wrong place for either, since what it means depends entirely on which backend reads it.

The same holds for the variable a hook templates against. `$collection` is a lowest common denominator in a notification body about mail, where `$mailbox` is what the person writing it would say, and in one about a calendar, where `$calendar` is.

This is the same argument as [watch-under-backend](../watch-under-backend/proposal.md) and [hook-under-backend](../hook-under-backend/proposal.md), applied to the one key those two left behind. With it moved, an account block carries nothing that needs a backend to be understood.

## What

Each backend takes the collection it watches, required, under the name its own domain uses: `imap.mailbox`, `jmap.mailbox`, `maildir.mailbox`, `caldav.calendar`, `carddav.addressbook`, `dav.collection`. The account-level `collection` goes, and with it the `mailbox` alias it carried.

A hook templates against the same name its backend configures, so a mail hook says `$mailbox`, a calendar hook `$calendar`, an addressbook hook `$addressbook` and a plain DAV hook `$collection`. `$id` is unchanged, being the one thing every backend means the same way. A hook naming another backend's word is refused when the file is read, the same as any other variable its event cannot fill.

CalDAV and CardDAV go domain-specific rather than stopping at `collection`, since a backend that already fires `on-event-*` and `on-card-*` has no reason to describe what holds them in the generic word. Only a plain DAV collection keeps `collection`, having no domain to name.

## The himalaya compatibility claim is already false, and this does not break it further

The module header, the README and the spec all say one file backs carillon, himalaya CLI and himalaya TUI. That stopped being true when `watch` moved under the backends: himalaya's `ImapConfig` is `deny_unknown_fields` and has no `watch` and no `hook`, so it refuses a carillon config today. It fails the other way too, carillon's `ImapConfig` being equally strict and having no `alpn` and no `sort`.

So this change adds a third refused key to a block that is already refused, and the claim has to be corrected either way. It is corrected here rather than quietly left standing.

Restoring the promise is a separate decision worth taking on its own terms: dropping `deny_unknown_fields` from carillon's backend blocks would do it, and would cost nothing this repository built, since the load-time refusals live on the nested `watch` and `hook` tables rather than on the block itself. What it would cost is catching `imap.serverr`. Not decided here.
