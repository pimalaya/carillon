---
cairn: log
change: a-refusal-carries-its-code
landed: 2026-11-15
---

# Scripted tests for the Graph, Google Calendar and Google People refusal readers

a-refusal-carries-its-code landed their readers without a scripted test. Spec moved: none, the behaviour being unchanged.

- src/scripted.rs (tests only, `cfg(all(test, api))`): the scripted stream the Gmail tests carried, lifted so the four vendor API backends share it; src/gmail.rs's tests use it.
- src/msgraph.rs: a watch arming on a mailbox whose folder listing Graph answers 403 `ErrorAccessDenied` (`permission-denied`), 403 `ErrorQuotaExceeded` (`quota-exceeded`, the code outranking the status), 507 (`quota-exceeded`), 429 (`rate-limited`), and 404 `ErrorItemNotFound` (no code).
- src/gcal.rs: a watch arming on a named calendar whose listing is answered 403 `forbidden` (`permission-denied`), 429 `rateLimitExceeded` and 403 `quotaExceeded` (both `rate-limited`, the last read from `error.errors[]` alone).
- src/gpeople.rs: a watch arming on every contact whose listing is answered 403 `PERMISSION_DENIED` (`permission-denied`), 429 `RESOURCE_EXHAUSTED` and a 403 naming `RATE_LIMIT_EXCEEDED` in `error.details[]` alone (both `rate-limited`).

Each refusal reaches the reader as the client returns it, io-msgraph's, io-gcal's and io-gpeople's own `ClientStdError::Send` read off the scripted wire. Mutating the readers (Graph's returning no code, Calendar's dropping `reasons`, People's dropping `detail_reasons`) failed the tests depending on the part mutated (Graph's three coded ones, the two rate-limit ones), the mutation then undone.
