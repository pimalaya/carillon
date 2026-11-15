---
cairn: tasks
change: a-refusal-carries-its-code
---

# Tasks

## Working rules

- Starts once the-family-line has landed.
- Read first: this change's proposal.md and delta.md; cli/cairn/log/2026-11-13-a-refused-line-exits-two.md and cli/src/error.rs (`CodedError`, `code_of`, `PartialOutcome`, `ErrorReport::eval_coded`); himalaya/tests/exit.rs and himalaya/src/account/check.rs (the `{backend, ok, code, error}` row and exit 3); neverest/src/error.rs (`ErrorCode::of_pimdir`, `code`, `refusal_code`) and its readers (`refusal_code` in neverest/src/imap/backend.rs, src/dav/client.rs, src/msgraph/client.rs, src/gmail/client.rs; `google_code` in src/throttle.rs).
- Every cargo command runs as `nix develop --command cargo ...`. `cargo fmt` after each phase.
- An agent edits, builds and tests; it runs no git write command. The session owning the repository commits each phase with the message given here, no AI trailer.
- Never adjust production code to fit a test. A regression test must fail against the code before the phase.
- No em dashes; code comments wrap at 80 columns; markdown is never hard-wrapped.
- Phase checks: as the-family-line's (all features, default, none, each backend alone; `sh cairn/verify.sh`).

## Phase 1, the exit contract (commit `feat!: a refused command line exits 2, a failed check exits 3`)

- [x] src/main.rs: `pimalaya_cli::clap::parse::<Cli>()`, `ErrorReport::<ErrorCode>::eval_coded`.
- [x] src/error.rs (new): `ErrorCode` with the whole vocabulary of the proposal (declared in every build, `allow(dead_code)` gated as neverest gates it; unconditional in Phase 1, where no reader constructs a code, for Phase 2 to gate), `error::code` reading `code_of` alone until Phase 2 adds the readers.
- [x] src/check.rs:235-242: `{backend, ok, code, error}`; :171 returns `PartialOutcome::default()` after printing when a row failed; the text output (:262-272) unchanged.
- [x] src/watch.rs:117: the Ctrl+C notice off stdout.
- [x] src/json_schema.rs: `carillon-error` and `carillon-check` follow; the registry test passes.
- [x] tests/exit.rs (new): exit 2 with `usage-invalid`, exit 2 without `--json` on stderr only, `check` exit 3 with a failed Maildir row.
- [x] Spec: Phase 1's part of delta.md folded (the stable code requirement without its table, its protocol reading, its DAV 403 and watch warning sentences and its store scenario, left for Phase 2).
- [x] Phase checks pass.

## Phase 2, the codes (commit `feat: code a server's refusal and a store's`)

- [x] `error::code` walks the chain over the readers below; the store codes reach it through an `of_pimdir` table.
- [x] Readers: IMAP (src/imap.rs; :393 keeps the typed cause), DAV (src/dav.rs, the inner io-webdav error types of :986 and :1188), Graph (src/msgraph.rs), Gmail, Google Calendar and Google People (src/gmail.rs, src/gcal.rs, src/gpeople.rs, a `google_code` copy), JMAP (src/jmap.rs, if io-jmap 0.9's client error exposes the status), pimdir (src/pimdir.rs:87 keeps the `PimdirError`).
- [x] src/watch.rs:239: the warning names the code when there is one.
- [x] Tests: the scripted IMAP server (src/imap.rs:466+) `NO [NOPERM]` and `NO [OVERQUOTA]`; the DAV scripted answers (src/dav.rs:1321+) a 403 bare, a 403 naming `valid-calendar-data`, a 507; Gmail's scripted stream (src/gmail.rs:389+) a 403 `insufficientPermissions` and a 429; `check --json` over a store of another draft coded `store-version`, exit 3.
- [x] Spec: Phase 2's part of delta.md folded.
- [x] Phase checks pass.

## Landing

- [x] CHANGELOG (two **BREAKING**, Added, Fixed), MIGRATION.md, README exit statuses, the src/main.rs header.
- [x] Log cairn/log/<date>-a-refusal-carries-its-code.md (frontmatter `cairn: log`, `change`, `landed:`); status landed; `sh cairn/verify.sh` clean.
- [ ] The live suites run by the owner.
