---
cairn: delta
change: bump-pimalaya-libs
---

## ADDED Requirements

## MODIFIED Requirements

### Requirement: The frontend owns transport and reconnect
The daemon SHALL own the transport: it opens the TLS connection (it trusts the user's own config, so it applies no SSRF guard) and hands the watch the stream. It SHALL open that stream with no retry strategy, taking back the not-ready failures a stream would otherwise retry away, since it arms a read deadline precisely to be woken up and poll its shutdown flag between IDLE keep-alives. It SHALL own the reconnect loop, resolving a fresh credential and opening a fresh connection on each attempt, with capped exponential backoff and jitter. The watch coroutine SHALL only run one session over the stream it is handed.

#### Scenario: The connection drops
- **GIVEN** a running watch
- **WHEN** the one-session watch returns because the connection dropped
- **THEN** the daemon waits a jittered backoff and reconnects, resolving the credential again

#### Scenario: Ctrl+C during a quiet IDLE
- **GIVEN** a watch idling against a server that has nothing to say
- **WHEN** the user presses Ctrl+C
- **THEN** the next read deadline expires as a failure the daemon sees, and the watch winds down rather than waiting for the server

## REMOVED Requirements
