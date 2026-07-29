---
cairn: log
change: standard-structure
landed: 2026-07-29
---

# Rebuild the CLI on the standard Pimalaya structure over the I/O-free core

Rewrote the CLI from scratch on the standard Pimalaya CLI layout (as in himalaya, ortie, sirup, mirador), driving carillon-core's now I/O-free watch coroutine with a blocking pump. The earlier async CLI was discarded: once core became I/O-free, the CLI wants no async runtime, and a blocking socket with one thread per watch is simpler.

## What landed

The toolkit crates: pimalaya-cli (the `Cli` parser, `StdoutPrinter`, `Logger`, `ErrorReport`, the shared `account`/`json`/`log` flags, `long_version!`, `manuals`/`completions` commands, and the build helpers), pimalaya-config (`TomlConfig`, `Secret`, shell-command deserialization), and pimalaya-stream (`StreamStd::connect_tls`, a blocking TLS stream). The nix flake, `build.rs`, and `deny.toml` follow the house shape.

Modules: main dispatches; cli declares the parser and the `watch` / `manuals` / `completions` subcommands; config parses `[accounts.<name>]` with an `imap` block (server, login, a `password` or `token` secret), an optional `mailbox`, and a `hook`; watch is the command; driver is the blocking pump; hook runs a content-free desktop notification and shell command.

The driver is the whole I/O side. It opens a TLS stream, constructs a `CarillonImapWatch`, and loops on `resume`: on `WantsRead` it reads (with a one-second timeout so a Ctrl+C is noticed), on `WantsWrite` it writes, on `Changed` it mints a `CarillonEvent` and fires the hook, on `Done` it returns. It reconnects with capped backoff, and mints the event's `id` and `ts` (the effects core does not perform). Green on check, clippy, and fmt via the crate's own flake.

## Deviations and notes

Initially the manifest pinned pimalaya-cli / config / stream to `0.1` behind a git `[patch.crates-io]` (crates.io was stale) and `build.rs` emitted `GIT_DESCRIBE` / `GIT_REV` directly, because `git_envs()` panicked on a tagless repo. Both are now resolved (2026-07-28): the crates are published (`cli 0.1.3`, `config 0.1.1`, `stream 0.1.2`) so the patch is dropped, and a root tag lets `build.rs` call `git_envs()` normally.

## Deferred

OAuth minting (only a pre-minted `token` is accepted, not a refresh flow). CardDAV, once core grows the poll coroutine. The hook is content-free by design: it never sees message content, only the account, mailbox, source, and state.
