---
cairn: log
change: family-housekeeping
landed: 2026-11-15
---

# Family housekeeping

Lines X1, X2 and X6 of the family's CLI alignment round (pimalaya/.github cairn/changes/cli-alignment-2026-11-15), carillon's part.

Every landed change moved from cairn/changes/ to the new cairn/changes/archive/, as in cardamum's cairn: all 40, an-alarm-keyed-by-its-listed-id among them, so changes/ holds in-flight proposals only. the-family-line names cli-alignment-leftovers by path and points into the archive. AGENTS.md and CONTRIBUTING.md say a landed change moves there, and CONTRIBUTING.md names cairn/verify.sh, which reports the cairn conformant.

MIGRATION.md's headings read "From v0.2 to v1.0" and "From v0.1 to v0.2", as himalaya, cardamum and calendula write them; the two task lines quoting the first follow.

.github/workflows/tests.yml gains the `features` job in cardamum's shape: clippy with `-D warnings` over the default features, `--all-features`, no backend, the four sets CONTRIBUTING.md lists (imap with rustls-ring, jmap with native-tls, maildir with no TLS provider, dav with rustls-aws) and each backend alone with rustls-ring, then the tests of a build without imap, which tests/backend_not_built.rs needs.

pimalaya-config moves to 0.5.2, required by both dependency tables of Cargo.toml, the Cargo.lock diff that crate alone: an unknown account key's error no longer lists `folder`, `mailbox`, `collection`, `watch`, `hooks` and `dav`, which the loader refuses as removed. Seen on the debug binary.

The standard commands already came in himalaya's order with pimalaya-cli's abouts (cli-alignment-2026-11-15, R8); README.md now names them in that order too.

The failure code table stays in cairn/spec/daemon.md: carillon's spec is that one capability file, which holds its commands, outputs and exit statuses, and has no commands spec to take it. tests/ already holds exit.rs, first_run.rs and account.rs.

Docs and build only, no proposal folder; no spec moved.
