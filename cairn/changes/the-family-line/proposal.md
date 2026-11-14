---
cairn: change
id: the-family-line
status: landed
created: 2026-11-14
---

# The family line

## Why

Carillon sits a family release behind the other apps:

- **pimalaya-cli 0.3** (Cargo.toml:67, lock 0.3.1). pimalaya-cli 0.4.0 (2026-11-11) removed the wizards, the `wizard`, `imap` and `jmap` features carillon's `wizard` feature names (Cargo.toml:30), `pimalaya_cli::wizard::keyring` (src/wizard/secret.rs:15) and the re-exports `commands::{CompletionCommand, JsonSchemaCommand, ManualCommand}` (src/cli.rs:21). Its `ConfigFlags` take one path per `-c`, never split on `:`, and read no environment variable.
- **pimalaya-config 0.3** behind a git pin (Cargo.toml:68, :79, 09f5060, redundant since 0.3.1). 0.4 requires `TomlConfig::sample_url` and `accounts`, drops `take_default_account` and `toml::to_string`, and answers `take_account` with an `AccountError` (src/config.rs:145-163, :272; src/check.rs:64); 0.4.1 lets a prompting secret command read the terminal.
- **pimalaya-stream 0.3** and the io-* releases of 2026-11-13 on it (cairn/log/2026-11-13-io-family-releases.md). The family moved to pimalaya-stream 0.4 the same evening: io-http 0.8, io-oauth 0.7, io-imap 0.10, io-jmap 0.9, io-msgraph 0.9, io-gmail 0.8, and on master io-webdav 185ab63, io-gcal dbb4481, io-gpeople e4a9cac.
- **io-pimdir 0.9**, pimdir draft-08 (Cargo.toml:61). Neverest, Himalaya, cardamum and calendula write and read draft-11 stores (io-pimdir 0.12), and every io-pimdir profile refuses another draft, so a `pimdir` watch over a store today's neverest wrote likely fails with `store-version` (read from io-pimdir's changelog, not run).
- **The wizard** (src/wizard/, 13 files, about 2,700 lines; the `configure` command, alias `wizard`, src/cli.rs:89-92; the first-run offer, src/cli.rs:116-130, :168-225; `config::render`, src/config.rs:259; the `carillon-configure` schema, src/json_schema.rs:24-25). The family removed every wizard (himalaya, neverest, cardamum, calendula, comodoro, ortie: each a change named the-wizard-leaves or wizard-removed): a configuration is written by hand from the documented sample, and a person missing one meets pimalaya-cli's welcome. It also holds carillon's only io-pim-discovery use, the last family crate on pimalaya-stream 0.3, which would leave two stream crates in the graph.

A second change, a-refusal-carries-its-code, follows this one and needs it: the failure contract of 2026-11-13 and 2026-11-14 rests on pimalaya-cli 0be1b1f.

## What

Two phases, each one commit, in this order. Neither needs anything outside this repository: dropping the wizard drops io-pim-discovery, so carillon does not wait on its stream-0-4 change.

### Phase 1, the wizard leaves (on today's libraries)

- Delete src/wizard/ and its `mod` (src/main.rs:2-3); the `configure` command and its alias; the `wizard` feature (Cargo.toml:30) and the io-pim-discovery dependency (:62) it alone pulls; `config::render` and its `toml::to_string` call (src/config.rs:259, :272); the `carillon-configure` schema key (src/json_schema.rs:24-25) and its registry entry.
- The bare invocation and a missing configuration follow Himalaya (himalaya/src/main.rs:20, :41-49, the-first-run-welcome) and Comodoro (comodoro/src/main.rs, `welcome::{is_wanted, shipped_sample}`): a bare `carillon` prints the help; a command finding no configuration fails naming the path it looked for and the documented sample, and a person at a terminal meets pimalaya-cli's welcome above that failure. The handling moves from `Cli::execute` (src/cli.rs:112-138) to src/main.rs, as Himalaya meets it (cli-alignment-leftovers item F6). The welcome arrives with pimalaya-cli 0.4, so Phase 1 prints the plain failure and Phase 2 adds the welcome.
- README: the wizard sections (:41, :46, :114, :125) give way to writing the configuration from config.sample.toml. The src/main.rs header (:22-29) and CONTRIBUTING.md (:36, "There is no test suite for a watch", stale) follow.
- Spec: "A first account is generated, never hand-written from scratch" is REMOVED; "A first run points at the sample" ADDED.

### Phase 2, the libraries

- Cargo.toml:
  - `pimalaya-cli` 0.4 and its build-dependency, features `terminal` and `spinner` (the `welcome` module included; check its feature gate); `[patch.crates-io]` `pimalaya-cli` 0be1b1f08d19ccb9d16ee8e1d4266c27c96e0cdd (the commit Himalaya and neverest pin; the second change needs its `clap::parse`, `usage_error` and `PartialOutcome` from 3);
  - `pimalaya-config` 0.4 from crates.io, its git pin (:79) dropped;
  - `pimalaya-stream` 0.4, `io-http` 0.8, `io-imap` 0.10, `io-jmap` 0.9, `io-msgraph` 0.9, `io-gmail` 0.8, `io-pimdir` 0.12; dev `io-oauth` 0.7;
  - `[patch.crates-io]` `io-webdav` 185ab63be5859c5ef6115bef97e4df5cb1187973, `io-gcal` dbb4481cd77ef65ca451420cb4e087fdccfe6fdb, `io-gpeople` e4a9cac80d023688118dbae88f8535145e8d2c9b (until io-webdav 0.9, io-gcal 0.5 and io-gpeople 0.8 release; taken from crates.io instead if they are out);
  - `io-maildir` 0.4, `io-sasl` 0.2, `ical-rs` 0.7 unchanged; the `ceichas` pin (661be05) kept, ceichas linking no pimalaya crate.
  - Then `cargo update`.
- Code the bump breaks:
  - src/config.rs:744: `server: self.cert.map(|cert| TlsServer { cert }), client: None`; the `tls.cert` keys (config.sample.toml:62 and their `jmap` and `caldav` twins) keep their names.
  - src/config.rs:145-163 `impl TomlConfig`: `sample_url` (the config.sample.toml URL on GitHub, as neverest's), `accounts`, `take_default_account` dropped. src/check.rs:64 and src/watch.rs:179-182 resolve the account through `take_account` and its `AccountError`, one wording for both (cli-alignment-leftovers item F2).
  - src/cli.rs:59-65: `-c` through pimalaya-cli's `ConfigFlags`: one path per `-c`, repeated to layer files, never split on `:`, and `CARILLON_CONFIG` no longer read, as cardamum's and Himalaya's.
  - src/cli.rs:21: the completion, json-schema and manual commands imported from their own pimalaya-cli modules.
  - io-pimdir 0.12: the reader calls (src/pimdir.rs: `PimdirReader::open`, `collection_kind`, `change_cursor`, `blobs`, `list_items`, `collections_changed_since`, `items_changed_since`, `get_item`, `PimdirBlobs::get`, `PimdirChangeCursor`'s `store_id`, `incarnation`, `purges` and `changed`) appear in no breaking line of 0.10 to 0.12; the test fixtures build `PimdirAction::Remove { seq, if_match, resolves }` (src/pimdir.rs:410). io-pimdir 0.11 counts a collection delete in `purges`, which carillon already reads as a reset (src/pimdir.rs:163-166).
  - The others (io-http, io-imap, io-jmap, io-msgraph, io-gmail, io-webdav, io-gcal, io-gpeople) change only their dependencies.
- `cargo tree -d --all-features -e normal`: no pimalaya-stream, io-http, io-oauth or io-pimdir twice.
- A test: a draft-11 store (made through io-pimdir 0.12's `PimdirStore`, as src/pimdir.rs:262-418's tests make theirs) is watched: its alarm fires.
- CHANGELOG Unreleased: **BREAKING** the wizard, `configure` and the first-run offer removed; **BREAKING** `-c` takes one path per flag and `CARILLON_CONFIG` is not read; **BREAKING** a pimdir watch reads draft-11 stores, the ones today's neverest writes, and refuses draft-08 ones; Changed the dependency line; Fixed the prompting secret command. MIGRATION.md is created (carillon has none), "From 0.2 to the next release": write the configuration by hand from config.sample.toml; split a `-c a:b` into `-c a -c b`; drop `CARILLON_CONFIG`; upgrade neverest's store.

## Decisions

- **The wizard leaves with the update**: pimalaya-cli 0.4 removed what it is built on, and the family removed every wizard. Keeping it would mean porting 2,700 lines onto APIs that no longer exist, and keeping io-pim-discovery on its own stream crate.
- **Two commits**: the wizard goes first, on the libraries it was written against, so the library commit changes only what the libraries break.
- **`-c` and `CARILLON_CONFIG` follow the family** (pimalaya-cli's one-config-path-per-flag, 2026-11-11).
- **Draft-11 only**: carillon reads the store as neverest writes it; a draft-08 store is reinstalled by neverest (its MIGRATION.md).
- These decisions are proposals: the owner reviews this change before any phase starts.

## Scope / non-goals

- The configuration sample's test and its shipping in share/doc (the family's the-sample-is-tested-and-shipped) are another change.
- cairn/changes/cli-alignment-leftovers (untracked, `status: active`, 2026-10-18) is mostly answered here: F2 by pimalaya-config 0.4's `take_account`, F6 by the move to src/main.rs, H1 and A2 obsolete (no wizard, no io-webdav pin left of its own). G2 (a fresh secret resolver per reconnect) stays open. The owner decides whether to mark it abandoned, pointing at this change; the agent does not edit that untracked folder.
- The active change hook-shape-follows-siblings is untouched.
- No version bump; no release.

## Order and parallel work

This change and a-refusal-carries-its-code run one after the other in this repository, in parallel with cardamum's and calendula's a-result-carries-its-errors, ortie's a-refusal-carries-its-code and comodoro's a-refused-line-exits-two, which touch other repositories.
