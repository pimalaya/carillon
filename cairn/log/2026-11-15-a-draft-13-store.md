---
cairn: log
change: a-draft-13-store
landed: 2026-11-15
---

# A draft-13 store

Line X14 of the family's CLI alignment round (pimalaya/.github cairn/changes/cli-alignment-2026-11-15), carillon's part.

pimalaya-cli moves to 0.6.0, io-imap to 0.11.2, io-gpeople to 0.8.1, io-msgraph to 0.9.2, io-gcal to 0.5.1 and io-pimdir to 0.14.0 (pimdir draft-13), each required at that version from crates.io. The Cargo.lock diff is those six crates.

A pimdir watch now reads draft-13 stores and refuses a draft-11 one coded `store-version` (cairn/spec/daemon.md, "A pimdir calendar is watchable"). The reader calls it makes and `PimdirError` did not move, so no code changed. io-imap 0.11.2 keeps an IMAP watch off the QRESYNC path unless the server's `ENABLED` names QRESYNC. pimalaya-cli 0.6.0 moves only `listing::Listing`, which carillon does not print; the io-gcal, io-msgraph and io-gpeople changes are writes carillon never makes.

tests/exit.rs `a_check_over_a_store_of_another_draft_is_coded_store_version` stamps the store `storage-draft-11`, failing on the code before.
