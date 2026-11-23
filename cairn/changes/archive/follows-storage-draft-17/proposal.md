---
cairn: change
id: follows-storage-draft-17
status: landed
created: 2026-11-23
---

# carillon follows storage-draft-17

## Why

neverest on io-pimdir master 7f16b27 writes storage-draft-17 stores; carillon 0.2.0 (io-pimdir 0.19.1, storage-draft-15) refuses them, so its pimdir backend stops firing alarms the day a user upgrades neverest. Every user then moves the store aside and syncs a new one, and that path meets two defects of the pimdir watch, both reproduced on carillon HEAD (ed02e89) and on the pinned build: a store replaced under a running watch is never read (the reader holds the old file), and once a session reads the new store an alarm fired in the last 15 minutes fires again (inventory.md B2, B5).

The shared frame (target, pin, git, CPUs, locks, accounts, decision rules, plan contents) is /home/soywod/code/pimalaya/pimdir/cairn/changes/the-consumers-follow-draft-17/frame.md; this file holds only what is carillon's. Lane CPUs 8,9,24,25, scratch ~/code/pimalaya/.pimdir-migration/carillon/ (seeder, harness, stores).

## What carillon touches

carillon is a reader (src/pimdir.rs): `PimdirReader::open`, `change_cursor`, `collection_kind`, `collection_account`, `list_items`, `items_changed_since`, `collections_changed_since`, `get_item` and `PimdirBlobs::get`; it takes no lock, writes nothing, stages no queue row (src/error.rs names queue codes only as the relayed vocabulary), and reads no index. Feature `io-pimdir/client` alone: it parses bodies with its own ical-rs, so `calendar`, `search`, `indexer` and `mail` stay off. On the pin it builds with no error and no warning, its 188 tests pass, the 17 clippy sets and the no-imap suite pass: the API it calls did not move.

## Scope

In:

- A. The pin, the lock moving io-pimdir alone (never `cargo update -p io-pimdir`, which re-resolves windows-sys in five crates), the nix build (pimalaya/nix's `importCargoLock` with `allowBuiltinFetchGit`, as neverest's and himalaya's pins), cargo-deny (the git source passes `allow-org.github`; RUSTSEC-2026-0592 on url 2.5.8 is red at HEAD too, D6), MSRV 1.89 unchanged.
- C. The refusal names carillon (B1): `PimdirFormatRefusal` with `program: "carillon"`, as himalaya, cardamum and calendula do; tests/exit.rs stamps the draft users hold.
- G. B2, B5, B6 (inventory.md), each test first.
- H. P1 (a re-read reads only what moved), P2 (no sleep steps in the poll, the backoff or the idle hook reaper: 25 wake-ups a second idle, 10 left for the signal loop), with the measures of inventory.md as baseline.
- I, J, K, L as tasks.md says.

Out:

- B (no compile break), D (no SQL on a moved table: tests/exit.rs's `UPDATE store_meta` alone, a column draft-17 keeps), E (no search, sort, query field, property or `pimdir` binary on carillon's path; the faster schema check and flag columns only shorten its reads), F (carillon never syncs).
- hook-shape-follows-siblings (the owner's, later), issue #7 (IMAP NOTIFY, off the pimdir surface), the main thread's 100 ms signal step (pimalaya-cli's `Shutdown` offers no blocking wait; a kit change), a change notification in place of the poll (latency is the interval by design, `pimdir.watch.poll.interval`).
- A collection-scoped change feed (needs-io-pimdir.md N1): carillon pages and filters every change of the store today, which it can keep.

## Decisions (defaults taken; the owner may reverse)

- D1. The refusal names carillon and the program's own draft (B1). Default: yes.
- D2. A replaced store is noticed by opening the reader on every poll (0.64 ms at 10k events, 0.83 ms at 100k, inventory.md), portable, rather than by comparing pimdir.db's inode (Unix only; on Windows SQLite holds the file and the move fails while carillon runs). Default: reopen.
- D3. A firing is keyed without the backend's item id (src/alarm.rs `AlarmFiring::key`), shared with CalDAV: two items of one calendar holding the same `UID` and alarm at the same trigger then fire once. Default: yes, the spec's "never fire one firing twice" read as one alarm instant.
- D4. `property-invalid`, io-pimdir's one new code, joins the relayed table (schema additive, never raised by a reader), to stay himalaya's table. Default: add it; drop it at K1 if himalaya's lane does not.
- D5. P2 changes the sleep of every polled backend (src/poll.rs) and the hook reaper (src/hook/command.rs), not only pimdir's path. Default: yes, the shutdown contract unchanged.
- D6. The url 2.5.9 lock bump lands in this change as its own commit, since every gate runs cargo-deny. Default: yes.
- D7. The alarm book's memory (about 5 KB an event: 61 MB at 10k, 517 MB at 100k) is left as it is; shrinking it reshapes the alarm engine CalDAV shares. Default: out of this change; P1 removes its doubling on a re-read.

## Risks

- The five CLIs must ship one io-pimdir revision (frame §2): K1 checks the pins before any release; carillon releases nothing on its own.
- A carillon older than this change, left running as a service while the user starts over, keeps firing the moved-aside store's alarms until restarted (B2); the CHANGELOG and README note say to restart it.
- D3 merges same-`UID` duplicates' firings; a calendar holding two such items is malformed by RFC 4791 §4.1.
- Measures were taken while other lanes built (inventory.md marks them); H3 re-runs them in a quiet window under the measure lock.
