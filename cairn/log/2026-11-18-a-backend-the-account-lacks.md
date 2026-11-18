---
cairn: log
change: a-backend-the-account-lacks
landed: 2026-11-18
---

# A backend the account lacks

Line COH-2 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4), carillon's part. Spec moved: daemon (The backend is picked by `-b`, a sentence and a scenario; A failure carries a stable code, a row).

- What COH-2 asked of carillon was already there: `-b` is a clap value enum listing its values in the help, the refusal and the completions, and a backend the build leaves out is `backend-not-built`, exit 1, as in himalaya. Re-checked on 44906ee.
- What was not: a pinned backend the account has no block for. On 44906ee `carillon --json -b imap account check` on a Maildir-only account failed uncoded ("No backend matching `imap` is configured for this account") and `watch` printed an uncoded `session-lost` ("Account has no `imap` config block") and retried it on its backoff for ever. neverest has no `-b`; himalaya, the precedent for the flag, fails it `capability-missing` with "Account `t` has no `pimdir` block" (himalaya/src/backend.rs, cairn/spec/commands.md). carillon now does the same through `error::backend_missing`: `ErrorCode::CapabilityMissing` joins the vocabulary, the ten `bail!` of `watch::watch_session` and the one of `account check` raise it, and `watch_account` stops the account for the run on it as on `backend-not-built`, the watch failing with it once no account is left. `-b auto` on an account declaring no block at all keeps its uncoded message, as himalaya's auto case does.
- Test: tests/account.rs `a_pinned_backend_the_account_lacks_is_capability_missing` (check and watch, the watch awaited for ten seconds) fails on the HEAD code (the check uncoded) and passes after.
