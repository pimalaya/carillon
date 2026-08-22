---
cairn: log
change: dav-backend
landed: 2026-08-22
---

# Watch a WebDAV collection, and stop naming events after mail

A fourth backend, and the vocabulary change it forced.

## What landed

`src/dav.rs` polls an RFC 6578 `sync-collection` report over any WebDAV collection, which is what a CalDAV calendar and a CardDAV addressbook are. The report asks for `getetag` and nothing else, so a poll carries no vCard and no VEVENT: the watch says a member moved, and a hook that wants its content goes and reads it. `dav.server` is the collection URL, `dav.auth` is basic, bearer or nothing, `dav.poll` the interval, a minute by default. It is behind a `dav` cargo feature, and `check` probes it with one report, which proves the transport, the credential and that the collection is where the config says.

RFC 6578 reports created and updated members together, so the backend keeps an href to etag picture of the collection and reads the difference: a member never seen is an arrival, a known member whose etag moved is an edit. A truncated report is drained straight away rather than at the next interval, since the rest is already waiting behind the token the server just handed back. A token the server refuses causes a re-enumeration, and that reports nothing, a re-baseline not being news.

That edit had nowhere to go. The four events were mail-shaped, and a message is immutable where a contact is not, so the vocabulary gained a fifth event and stopped being named after mail: `on-message-added` and `on-message-removed` are now `on-item-added` and `on-item-removed`, `on-item-changed` is new, and the old names still work as aliases, so no configuration already written breaks. `WatchEvent` follows, and the enum carries an allow for the variants a reduced feature set cannot construct, since which events exist is the vocabulary's business and which are reachable is the enabled backends'.

The connection is opened the way `prompt-shutdown` established: a read deadline and no retry strategy, so a poll against a server that stopped answering ends at the next deadline rather than holding the thread.

## Verification

Build, clippy and fmt green on every feature combination (imap, jmap, maildir, dav, and all four), and the five Maildir tests still pass. No CalDAV or CardDAV server has been watched yet; that is the open task on the change, alongside the same one for IMAP and JMAP.

## Not done

Discovery. The collection URL is pasted in rather than found from a bare domain through RFC 6764 and a home-set lookup, which io-pim-discovery already does for the other Pimalaya tools. It changes the config surface rather than the watch, so it fits as a follow-up.

## Follow-up: the flags question, and a documentation pass

Whether flags belong in a domain-agnostic tool at all was raised, and pimdir answered it: `flags` is a column on every item whatever its kind, holding "a JSON array of the raw flag strings", with `NULL` meaning unknown and `'[]'` meaning known-empty, and the per-kind detail living in the separate opaque `meta`. So flags stay, under their name, and mail is simply the kind that populates them today.

Two things followed from reading that spec. The unknown-versus-empty distinction is now stated: a WebDAV poll reads etags, so an item's flags are unknown to it rather than empty, which is why it reports none. And `MessageSummary` became `ItemSummary`, described as what pimdir calls `meta`, which leaves a place for a DAV resolver to put a contact's name later.

What was considered and dropped: reporting raw flag strings the way pimdir stores them. A store keeps them raw because it has to round-trip exactly what the server said; a notifier does not, and "a filter written once fires on every backend" is a stated feature here. The normalisation stays.

The documentation was read back and the stale parts fixed. The one that mattered: the sample config told a JMAP user to set `mailbox` to the Mailbox **id**, while the resolver matches by name (falling back to the special-use role for `INBOX`), so following that advice would have failed to resolve. Also corrected: the migration guide still counted three backends and four message-shaped hooks, the changelog described the hooks under their old names, and the `mailbox` field said nothing about the `dav` backend ignoring it. The module headers were trimmed, and `src/main.rs` regained the architecture header AGENTS.md points at.
