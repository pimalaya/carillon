---
cairn: change
id: cli-alignment-leftovers
status: landed
created: 2026-10-18
---

# What carillon still owed the family alignment

## Why

The cross-repo alignment audit of 2026-10-18 (CLI_ALIGNMENT_PLAN.md, which kept the item ids of the 2026-08-28/29 plan) found five items open in carillon: F2 (two wordings for an account that cannot be resolved), F6 (the bare invocation met inside `Cli::execute` rather than in `main`), G2 (a fresh secret resolver on every reconnect), A2 (the io-webdav git pin) and H1 (the wizard's local copy of a save layer pimalaya-cli was to ship).

## What

Re-checked against the code on 2026-11-15, nothing behavioural is left: each item was closed by another change or is closed by a decision.

- **F2, done** by the-family-line (d3f41c2): `watch -a` and `check` both resolve the account through pimalaya-config's `take_account` (src/watch.rs `select_accounts`, src/check.rs), one `AccountError` wording for both, an unknown name listing the accounts held. Bare `watch` still watches every account, which the spec requires.
- **F6, done** by the-family-line (5388407): `Cli::execute` is gone; src/main.rs meets a bare `carillon` with the help in its `execute` and a missing configuration with pimalaya-cli's welcome in `main`, as himalaya's src/main.rs does today (the line 139 the audit cited no longer exists, the file being 79 lines).
- **G2, kept by design** (owner, 2026-11-15): a watch builds a fresh `pimalaya_config::secret::SecretResolver` per connection it opens (src/dav.rs, src/gmail.rs, src/gcal.rs, src/gpeople.rs, src/msgraph.rs, src/jmap.rs, src/imap.rs). pimalaya-config documents that the resolver holds the plaintext for as long as it lives and should be dropped with the runtime account it was built for rather than kept for the life of the process; a watch lives as long as the process, so one resolver per connection is that rule applied, and it also hands a reconnect a fresh broker token. `check` keeps one resolver for the whole account, which lives no longer than the command. The spec already says both ("Credentials SHALL be resolved per attempt rather than held", "The resolver SHALL live no longer than the account it was built for"), so nothing moves.
- **A2, handled at release** (owner, 2026-11-15): the owner releases the libraries and drops every `[patch.crates-io]` pin after the 1.0 round, io-webdav's (now 185ab63) among them; nothing is done here.
- **H1, moot**: the wizard and src/wizard/ were dropped by the-family-line (5388407), so there is no local save layer left to hand to pimalaya-cli.

No spec requirement changes, so this change carries no delta.
