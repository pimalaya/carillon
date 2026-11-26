---
cairn: log
change: pimdir-1-0-codes-relayed
landed: 2026-11-26
---

# carillon relays io-pimdir 1.0.0's `source-empty`

io-pimdir 1.0.0 added two codes: `source-empty` (`PimdirError::SourceEmpty`, a store bound to a source of an empty id) and `target-unsynced` (`PimdirMutateError::Unsynced`, a copy or a move into a collection no source syncs). carillon's relay table (src/error.rs `PIMDIR_CODES`) follows himalaya's, which takes `source-empty` in the same 1.0.0 round, so carillon relays it too, where 2026-11-26-io-pimdir-1-0 left it out while himalaya's did.

- **Code**: `ErrorCode::SourceEmpty`, serialised `source-empty`, in the `carillon-error` schema. carillon, a reader, never meets it; the entry keeps the one vocabulary every pimdir app reads.
- **Not relayed**: `target-unsynced`. It is `PimdirMutateError::code`, raised only on the owner's run, which carillon never drives. himalaya leaves it out too, so the two tables stay equal.
- **Tests**: `the_relayed_pimdir_codes_are_himalayas` counts 31 entries and reads `PimdirError::SourceEmpty` as `source-empty`; `the_failure_code_is_typed_by_both_vocabularies` finds it in the schema.
- **Docs**: CHANGELOG 1.0.0 (the rest of io-pimdir's codes), the relayed row of daemon.md's code table.

Capabilities moved: daemon (one code).
