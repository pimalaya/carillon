---
cairn: change
id: absorb-core
status: landed
created: 2026-08-22
---

# Absorb carillon-core into the daemon

## Why

carillon-core was split out so two frontends could run one watch loop under two I/O models: the CLI blocking with a thread per watch, the server async over many connections. With the server postponed, the CLI is the only frontend, and the split now costs a repository, a CI, a flake, a Cairn tree and a git dependency for a benefit nobody collects.

The benefit was never reuse in the abstract, so nothing is lost by folding it back. What must survive the fold is the property that made the split good: the watch stays an I/O-free coroutine, so reviving the server means writing an async driver, not a second watcher.

The trims fall out of the same reasoning. `CarillonTransportClass` exists so a frontend can refuse a public-callback source it cannot host; with no server, there is nothing to refuse. The CardDAV poll has no consumer here (the CLI never enabled the feature) and returns with the server. `CarillonEvent`'s id and timestamp exist so a delivery can be signed and deduplicated; a desktop notification needs neither, and the compiler said so.

The fuzz target goes for a different reason, and it would have gone even with the server here: every response byte is parsed by imap-codec, which fuzzes itself and says so in its README. Carillon's target fed its input at the first read and EOF thereafter, so it never advanced past the greeting: it was re-fuzzing imap-codec's greeting parser through three layers of indirection. The parser bound this crate picks is its own, so that one assertion stays as a unit test.

## What

- The watch modules (imap, backend, credential, event, error) move into the daemon crate beside the frontend ones.
- `CarillonTransportClass`, `CarillonBackend`, the CardDAV backend and poll, the event's `CardDav` source, `CarillonEvent` itself with its id and timestamp minting, and the `Serialize` derive the server's webhook payload needed, do not come along.
- The fuzz directory and the hostile-corpus test go. The oversized-literal test stays, since the bound is this crate's choice.
- The `carillon-core` dependency and its git source are gone.

## Consequences

carillon-server stops building until it is revived, since it depends on carillon-core. That is recorded in its own repository.
