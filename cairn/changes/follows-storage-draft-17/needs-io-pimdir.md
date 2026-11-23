---
cairn: change
id: follows-storage-draft-17
part: needs-io-pimdir
---

# What carillon needs from io-pimdir

Nothing blocks: carillon builds and passes on 7f16b27 unchanged, and works around both items below. Evidence in inventory.md.

## N1. A change feed scoped to one collection (low, optional)

- What: `PimdirReader::items_changed_since` restricted to one collection (an index on `items(collection, changed)`), or the feed's rows carrying the object hash.
- Why: a calendar watch pages every change of the store, mail included, 500 rows a page, and keeps its own collection's (src/pimdir.rs:210-237): after a first mail sync of a large store it reads every row once; the hash would also spare a `get_item` per changed event (read from code: `committed_item` runs `load_kind`, one statement per calendar summary table, three, and the addresses, src/client/reader.rs:2128-2160).
- Evidence: inventory.md, Performance: the poll after 10,000 mail changes costs 70 ms of CPU on both builds, so a first sync of the 517k bench store would cost one poll about 3.6 s (extrapolated, unverified).
- App test: H4's noise measure.
- Workaround: the current paging, kept.

## N2. A reader holds a store moved aside (information, no ask)

- What: a `PimdirReader` keeps reading the file it opened after the store directory is moved aside and a new store is synced under its path: its `change_cursor` never moves, no error comes (B2). Any long-lived reader (carillon, a TUI, the Android app's) meets it; STORAGE §8 could say a long-lived reader opens the store again to see a new one, or the reader could answer whether its path still names its file.
- Workaround in carillon: the reader is opened again on every poll (D2), 0.7 ms on a draft-17 store of 1k events.

## N3. Generator stores with alarms (low, optional)

- What: spec/generator renders events without a `VALARM` (src/body.rs `Event::file`), so no generated store exercises an alarm reader.
- Workaround: carillon's lane seeds its stores through `PimdirProducer` and the drain (scratch seeder, inventory.md).

## N4. A listing of public ids and object hashes alone (low, optional)

- What: a keyset page of `(seq, object)` for one collection, with no `PimdirItem` built (flags, sort key, link id), or `list_items` made cheaper per row.
- Why: after a reset, a purge anywhere in the store or the calendar moving, carillon lists its calendar again to find what changed (H2); at 10,000 events that listing is what is left of the poll: 2 to 3 ticks of CPU (20 to 30 ms) against 7 to 9 for the whole open, where the plan asked 5% of the open. A purge in a mail collection, which neverest does as it expunges, triggers it.
- Evidence: journal.md 2026-11-23 17:20, seeder measure-purge on the 10k store, before 9 to 10 ticks, after 2 to 3.
- App test: src/pimdir.rs `a_purge_elsewhere_reads_no_body_of_an_item_unchanged` (bodies), H4's purge measure (CPU).
- Workaround: the full `list_items` listing, kept.
