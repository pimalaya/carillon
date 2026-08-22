---
cairn: log
change: absorb-core
landed: 2026-08-22
---

# Absorb carillon-core into the daemon

carillon-core is gone from this repository's dependency list: its modules now live in the daemon crate. The split had one purpose, letting the CLI and the server run one watch loop under two I/O models, and with the server postponed the CLI is the only frontend paying for it.

## What landed

`src/imap.rs` (the coroutine), `src/backend.rs`, `src/credential.rs`, `src/event.rs` and `src/error.rs` moved in unchanged in behaviour, beside the frontend modules that already drove them.

Four trims rode with the fold, each with its own reason. `CarillonTransportClass` and `CarillonBackend` went: the class exists so a frontend can refuse a public-callback source it cannot host, and with no server there is nothing to refuse. The CardDAV backend, poll coroutine and `CardDav` event source went with the server that wanted them; the CLI never enabled the feature. `CarillonEvent` went once the compiler pointed out that four of its six fields were never read here: its id and timestamp exist so a delivery can be signed and deduplicated, which a desktop notification does not need, so the driver now hands the hook the account, mailbox, source and state it already holds. And the `Serialize` derive went with the webhook payload that needed it.

The fuzz target and the hostile-corpus test went too, for a reason independent of the server: every response byte is parsed by imap-codec through io-imap's coroutines, and imap-codec fuzzes itself. Worse, the target fed its input at the first read and EOF thereafter, so it could never advance past the greeting; it was re-fuzzing imap-codec's greeting parser through three layers. What is genuinely this crate's is the 1 MiB parser bound it picks, so the test that an over-bound literal ends the watch stays, and says why it stayed. The crate went back to a plain binary once the fuzz target no longer needed a library to link against.

The property that made the split worth having survives it: the watch is still an I/O-free coroutine, so reviving the server means writing an async driver rather than a second watcher. That is now a spec requirement rather than a property of a separate crate.

## Verification

Build, test, clippy and fmt green under the nix devshell.

## Spec

The daemon capability gained the I/O-free-watch requirement and, folded in from the retired core spec where they were already current truth, the content-free-ring and IDLE-plus-EXAMINE requirements. It lost the transport-class requirement. The prose now states the line the whole design hangs on: it signals, it never syncs, so there is no replica, no per-message delta, and no diff API.

## Consequences

carillon-server still depends on `carillon-core` by git and stops building until it is revived. That is recorded in its own repository.

## Revised the same day

The first landing of this change (commit 7307e15) brought the fuzz target and the hostile-corpus test along, and made the crate a library so they could link against it. Review caught that the target could not do what it claimed: parsing belongs to imap-codec, which fuzzes itself, and the target's own drive loop fed its input at the first read and EOF thereafter, so it never advanced past the greeting. It went, the corpus test went with it, and the crate went back to a plain binary. The bound test stayed. This entry describes where the change came to rest; the intermediate state is in the history above it.
