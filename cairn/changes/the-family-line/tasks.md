---
cairn: tasks
change: the-family-line
---

# Tasks

## Working rules

- Read first: this change's proposal.md and delta.md; the family's wizard removals (himalaya/cairn/log/2026-11-11-the-wizard-leaves.md, comodoro/cairn/log/2026-11-12-the-wizard-leaves.md, neverest/cairn/log/2026-11-13-wizard-removed.md); himalaya/src/main.rs (the welcome, :20, :41-49); pimalaya-cli's CHANGELOG (0.4.0 and Unreleased) and pimalaya-config's (0.4.0, 0.4.1); io-pimdir's CHANGELOG from 0.10.0 to 0.12.0; neverest commit c1837c5 (its dependency bump, `TomlConfig` and `Tls`).
- Every cargo command runs as `nix develop --command cargo ...`. `cargo fmt` after each phase.
- An agent edits, builds and tests; it runs no git write command. The session owning the repository commits each phase with the message given here, no AI trailer. The untracked cairn/changes/cli-alignment-leftovers is not touched.
- Never adjust production code to fit a test. A regression test must fail against the code before the phase.
- No em dashes; code comments wrap at 80 columns; markdown is never hard-wrapped.
- Phase checks, each one: `cargo test --all-features`; `cargo clippy --all-targets -- -D warnings` over the default set, `--all-features`, `--no-default-features`, and `--no-default-features --features rustls-ring,<b>` for each backend of CONTRIBUTING.md:28-31 (imap, jmap, maildir, dav, msgraph, gmail, gcal, gpeople, pimdir); `sh cairn/verify.sh`.

## Phase 1, the wizard leaves (commit `feat!: drop the wizard, write the configuration from the sample`)

- [ ] Delete src/wizard/ and `mod wizard` (src/main.rs:2-3); the `configure` command and its `wizard` alias (src/cli.rs:89-92); the `wizard` feature (Cargo.toml:30) and the `io-pim-discovery` dependency (Cargo.toml:62); `config::render` (src/config.rs:259+) and its `toml::to_string` (:272); `carillon-configure` (src/json_schema.rs:24-25).
- [ ] The first-run offer (src/cli.rs:116-130, :168-225) goes: a bare `carillon` prints the help; a command finding no configuration fails naming the path and config.sample.toml; the handling moves to src/main.rs.
- [ ] README (:41, :46, :114, :125), the src/main.rs header (:22-29) and CONTRIBUTING.md (:36) follow.
- [ ] Tests: a command with no configuration exits 1 naming the path and the sample (a new tests/first_run.rs on the built binary, as comodoro/tests/first_run.rs); the json-schema registry test (src/json_schema.rs:45-56) passes without `carillon-configure`.
- [ ] Spec: REMOVED and ADDED of delta.md folded.
- [ ] Phase checks pass.

## Phase 2, the libraries (commit `build: move to pimalaya-cli 0.4, pimalaya-config 0.4, pimalaya-stream 0.4 and io-pimdir 0.12`)

- [ ] Cargo.toml: pimalaya-cli 0.4 (dependency and build-dependency), pimalaya-config 0.4 (its git pin dropped), pimalaya-stream 0.4, io-http 0.8, io-imap 0.10, io-jmap 0.9, io-msgraph 0.9, io-gmail 0.8, io-pimdir 0.12, dev io-oauth 0.7.
- [ ] `[patch.crates-io]`, sorted, no comment: ceichas (kept), io-gcal dbb4481cd77ef65ca451420cb4e087fdccfe6fdb, io-gpeople e4a9cac80d023688118dbae88f8535145e8d2c9b, io-webdav 185ab63be5859c5ef6115bef97e4df5cb1187973, pimalaya-cli 0be1b1f08d19ccb9d16ee8e1d4266c27c96e0cdd; a crate released meanwhile from crates.io instead.
- [ ] `cargo update`; the lock holds pimalaya-config 0.4.1.
- [ ] src/config.rs:744 `Tls::server`; src/config.rs:145-163 `TomlConfig` (`sample_url`, `accounts`, no `take_default_account`); src/check.rs:64 and src/watch.rs:179-182 through `take_account` and `AccountError`.
- [ ] src/cli.rs:59-65: pimalaya-cli's `ConfigFlags` (one path per `-c`, no `CARILLON_CONFIG`); src/cli.rs:21 imports.
- [ ] src/main.rs: pimalaya-cli's welcome above the missing-configuration failure for a person at a terminal (`welcome::is_wanted`, `shipped_sample`, as himalaya and comodoro).
- [ ] io-pimdir 0.12: src/pimdir.rs builds; its tests' `PimdirAction::Remove { seq, if_match, resolves }` (:410); a test watching a store made by io-pimdir 0.12 fires its alarm.
- [ ] tests/common/mod.rs:274, :325 and tests/google.rs:261 (`Tls::default()` with io-oauth 0.7) build.
- [ ] `cargo tree -d --all-features -e normal`: no pimalaya-stream, io-http, io-oauth or io-pimdir twice.
- [ ] Spec: MODIFIED of delta.md folded.
- [ ] Phase checks pass.

## Landing

- [ ] CHANGELOG (the **BREAKING** entries, the dependency line, Fixed), MIGRATION.md created ("From 0.2 to the next release"), README.
- [ ] Log cairn/log/<date>-the-family-line.md (frontmatter `cairn: log`, `change`, `landed:`); status landed; `sh cairn/verify.sh` clean.
- [ ] Then a-refusal-carries-its-code starts.
- [ ] The live suites (`--test google --test msgraph --test jmap -- --ignored --test-threads 1`, Stalwart through tests/stalwart.sh) run by the owner.
