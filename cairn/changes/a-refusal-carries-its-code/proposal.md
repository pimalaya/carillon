---
cairn: change
id: a-refusal-carries-its-code
status: active
created: 2026-11-14
---

# A refusal carries its code

## Why

On 2026-11-13 and 2026-11-14 the family settled its failure contract:

- **The exit contract** (pimalaya-cli a-refused-line-exits-two, eb37962): 0 success, 1 the failure object, 2 a command line the grammar refuses, printed under `--json` as the failure object coded `usage-invalid`, 3 and above a partial outcome, among them a `check` with a failed backend. Himalaya, neverest, cardamum and calendula adopted it.
- **Stable codes**: a failure a caller acts on carries a code (cardamum, calendula and neverest relay the pimdir store's, `store-version` among them), and a server's refusal carries the shared `permission-denied`, `quota-exceeded` or `rate-limited`, read from the typed error of the protocol that refused (Himalaya's a-result-carries-its-errors, neverest's a-refusal-carries-its-code).
- **The check row**: every binary prints a backend check row as `{backend, ok, code, error}` (pimalaya-cli spec output, "Shared nouns keep one shape").

Carillon has none of it:

- src/main.rs:111-114 parses with clap's own `Cli::parse()` and reports through the uncoded `ErrorReport::eval`, so a refused line under `--json` prints clap's text on stderr, and every failure prints `"code": null`.
- `check` prints `BackendCheck { backend, ok, error }` (src/check.rs:235-242), no `code`, and returns `Ok` after printing a failed backend (src/check.rs:171), so it exits 0 where its own spec says non-zero ("The account can be checked before it is watched").
- The pimdir backend stringifies the store's error (`bail!("Cannot open pimdir store at {}: {err}")`, src/pimdir.rs:87), so `store-version` and the other io-pimdir codes cannot be relayed; the IMAP resolver does the same with io-imap's (`bail!("{err}")`, src/imap.rs:393).
- `watch`'s Ctrl+C handler prints on stdout (`println!`, src/watch.rs:117), which `--json` reserves for values.

Carillon has no command listing or acting on several items: `watch` is long-running and reports through hooks and logs, so `errors` lists and pimalaya-cli's `ItemError` do not apply.

This change follows the-family-line, which brings pimalaya-cli 0be1b1f (`clap::parse`, `usage_error`, `PartialOutcome` from 3) and io-pimdir 0.12 (whose `PimdirError::code` names the codes relayed here).

## What

Two phases, each one commit, in this order, after the-family-line.

### Phase 1, the exit contract

- src/main.rs: `pimalaya_cli::clap::parse::<Cli>()`; a refused line exits 2, under `--json` with the failure object coded `usage-invalid` on stdout. `ErrorReport::<ErrorCode>::eval_coded` replaces `ErrorReport::eval`.
- src/error.rs (new): `ErrorCode`, kebab-case, `Serialize` and `JsonSchema`, declared in every build; `error::code(err)` = `code_of(err)` else the first cause a protocol reader codes (Phase 2), as neverest/src/error.rs.
- `check`: each row is `{backend, ok, code, error}`, `code` from `error::code` (`null` when the backend passed or its failure carries none); when any row failed, the command prints the report and returns `PartialOutcome::default()`: exit 3.
- src/watch.rs:117: the Ctrl+C notice goes to stderr (or through the logger), never stdout.
- json-schema: `carillon-error`'s `code` typed by `ErrorCode` and pimalaya-cli's `usage-invalid`; `carillon-check` with the row's `code`.
- tests/exit.rs (new, on the built binary): `--json watch --nope` prints the coded object and exits 2; `--json check` on an account whose Maildir root does not exist prints the report, its row `ok: false`, and exits 3.

### Phase 2, the codes

- The vocabulary:

| Code | Raised when |
|---|---|
| `usage-invalid` | the command line does not match the grammar, raised by pimalaya-cli |
| `permission-denied` | the server refused the account: an HTTP 403 that is no rate limit (Graph, Gmail, Google Calendar, Google People, JMAP), a DAV 403 naming no precondition or `need-privileges` (RFC 3744 §7.1.1), an IMAP `NOPERM` (RFC 5530 §3) |
| `quota-exceeded` | an HTTP 507, a DAV `quota-not-exceeded` (RFC 4331 §6), Graph's `ErrorQuotaExceeded`, an IMAP `OVERQUOTA` (RFC 5530 §3) |
| `rate-limited` | an HTTP 429, or a Google 403 naming a rate limit or a quota; carillon backs off between sessions but retries no request |
| `store-version`, `store-uncreated`, `store-foreign`, `store-corrupt`, `store-not-current`, `store-hash-algo`, `store-exposed`, `sync-version` | io-pimdir's `PimdirError::code` for a store a reader meets, spelled as io-pimdir does (neverest's `ErrorCode::of_pimdir`); the owner's codes (`store-busy`, `store-owned`, `lock-replaced`) do not reach a reader taking no lock |

- One reader per protocol, `pub(crate) fn refusal_code(cause: &(dyn std::error::Error + 'static)) -> Option<ErrorCode>` beside its client, mirroring neverest's:
  - IMAP (src/imap.rs): `ImapClientError::status()`'s response code, `Code::OverQuota` and `Code::Other("NOPERM")`. The resolver's `bail!("{err}")` (src/imap.rs:393) keeps the typed error as a cause instead.
  - DAV (src/dav.rs): carillon drives io-webdav's coroutines itself, so the chain holds `WebdavSendError`, `WebdavSyncCollectionError` and `WebdavFollowRedirectsError` (src/dav.rs:986, :1188), not `WebdavClientStdError`: neverest's `refused` (neverest/src/dav/client.rs) is adapted to those inner types, their `HttpStatus { status, body }`.
  - Graph (src/msgraph.rs): `MsgraphClientStdError::Send(send)`, `send.api()`.
  - Gmail, Google Calendar, Google People (src/gmail.rs, src/gcal.rs, src/gpeople.rs): a copy of neverest's `google_code` with the parts of `google_refusal` it reads.
  - JMAP (src/jmap.rs): carillon only reads (`/changes`, `/get`, `/query`), so no SetError arises; an HTTP-level 403 or 429 is coded if io-jmap 0.9's client error exposes its status (to read in io-jmap; left uncoded and said so in the log otherwise).
  - pimdir (src/pimdir.rs:87): the `PimdirError` kept as a cause, coded through an `of_pimdir` table as neverest's.
- `watch`'s "session lost" warning (src/watch.rs:239) names the code when the failure carries one (`[work] session lost (permission-denied): …`), so a log reader tells a refused account from a network blip; nothing else in the watch changes.
- Tests, each through the real error path: the IMAP scripted server (src/imap.rs:466+) answering `NO [NOPERM]` and `NO [OVERQUOTA]`; the DAV scripted answers (src/dav.rs:1321+) a 403 with and without a precondition and a 507; Gmail's scripted stream (src/gmail.rs:389+) a 403 `insufficientPermissions` and a 429; `check --json` over a pimdir root holding a draft-08 store (or a store another draft stamped) prints the row coded `store-version` and exits 3.

### Docs

CHANGELOG Unreleased: **BREAKING** a refused command line under `--json` prints the failure object on stdout, still exiting 2; **BREAKING** `check` exits 3 when a backend failed, its rows gaining `code`; Added the codes; Fixed the Ctrl+C notice printed on stdout. MIGRATION.md ("From 0.2 to the next release", created by the-family-line): exit 3 and the codes. README: an exit status line. The src/main.rs header names the exit statuses.

## Decisions

- **Same id as neverest's and ortie's change**: carillon codes refusals without `errors` lists.
- **No `errors` list**: no carillon command lists or acts on several items; `watch` reports through hooks.
- **No hook on failure**: an error code has no hook to ride today (hooks fire on items and alarms, src/hook.rs:34-66); a `session-lost` hook carrying `$code` would be a feature of its own.
- **`rate-limited` without retries**: carillon reconnects with a capped backoff (src/watch.rs:81-88, :242-251) but sends no request twice, so the code names the refusal a session ended on.
- These decisions are proposals: the owner reviews this change before any phase starts.

## Scope / non-goals

- The active change hook-shape-follows-siblings is untouched.
- No version bump; no release.

## Order and parallel work

After the-family-line, in this repository; in parallel with the other apps' changes.
