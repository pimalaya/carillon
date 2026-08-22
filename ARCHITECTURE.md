# mirador architecture

Read the [Pimalaya ARCHITECTURE](https://github.com/pimalaya/.github/blob/master/ARCHITECTURE.md) first: it describes the conventions every Pimalaya repository shares (layering, the sans-I/O coroutine approach, command and config conventions, code style, licensing). This document only covers what is specific to mirador, and assumes you know that shared context.

If a statement here conflicts with the code, the code wins; please flag it.

## Where mirador fits

mirador is an **application**, the top layer of the Pimalaya stack: a CLI that watches a mailbox and fires hooks when it changes. It has no library target (only `main.rs`) and writes no protocol or storage logic of its own. It is a thin shell that drives the sans-I/O libraries below it:

- [io-email](https://github.com/pimalaya/io-email): the cross-protocol email domain API, exposed as the blocking `EmailClientStd`; mirador uses its `watch_mailbox` shared command;
- [io-imap](https://github.com/pimalaya/io-imap), [io-jmap](https://github.com/pimalaya/io-jmap), [io-maildir](https://github.com/pimalaya/io-maildir): the backends that actually implement watching;
- [pimalaya-cli](https://github.com/pimalaya/cli), [pimalaya-config](https://github.com/pimalaya/config), [pimalaya-stream](https://github.com/pimalaya/stream): shared CLI plumbing (clap args, printer, logger), TOML config loading, and the blocking I/O runtime (TLS, SASL).

All real I/O lives in those libraries; mirador consumes their blocking `*Std` clients and only orchestrates them and renders results. The binary is synchronous end to end (`std::net`, `std::thread`); there is no async runtime.

## The watch model

`watch` is the whole point of mirador. It calls io-email's `EmailClientStd::watch_mailbox`, which forwards envelope-level deltas as `WatchEvent`s. Each backend implements watching its own way: IMAP via [RFC 2177 IDLE](https://datatracker.ietf.org/doc/html/rfc2177), JMAP via [RFC 8620 §7.2 EventSource](https://datatracker.ietf.org/doc/html/rfc8620#section-7.2) push, Maildir via filesystem notifications. Not every backend can watch: Gmail and Microsoft Graph have no IDLE/push primitive wired into io-email, so they are deliberately absent here.

`watch_mailbox` is **blocking** and owns the connection for the session's lifetime, so `watch.rs` runs it on a dedicated worker thread and streams events back to the main thread through an `std::sync::mpsc` channel. The main thread loops on `recv_timeout`, dispatches each event to the configured hook, and polls a shared `AtomicBool` shutdown flag set by the Ctrl+C handler. On shutdown the worker observes the flag, winds the driver down cleanly (sends `IDLE DONE`, closes the SSE socket, drops the notify watcher) and returns; the main thread joins it and surfaces any error.

The five `WatchEvent` kinds (`EnvelopeAdded`, `EnvelopeRemoved`, `FlagsAdded`, `FlagsRemoved`, `KeepAlive`) map to the four hook slots; `KeepAlive` is a no-op that just proves the connection is live.

## Hooks

Each watch event kind has an optional hook (`config.rs`, `HooksConfig`). A hook can fire a desktop **notification** (via [notify-rust](https://crates.io/crates/notify-rust)) and/or run a **shell command** (`hook.rs`). Notification summary/body strings are expanded with [subst](https://crates.io/crates/subst) using shell-style `$name` / `${name}` placeholders (`id`, `mailbox`, and for added messages the sender/recipient/subject fields); command hooks instead receive those names as environment variables and let the shell expand them. Flag hooks accept an optional `flags = [...]` filter that narrows firing to specific IANA-classified flags. Hook failures are logged at `warn` and never crash the watch loop.

## Backend selection

The global `-b/--backend` flag drives a `Backend` enum (`backend.rs`): `auto` (default), `imap`, `jmap`, `maildir`. Mirador opens exactly one connection per `watch`, so `client::open` (`client.rs`) walks the account's configured-and-allowed blocks in priority order (IMAP, then JMAP, then Maildir) and registers the first match onto a fresh `EmailClientStd`; a named `--backend` pins the choice and bails when that block is absent. This mirrors himalaya CLI's `Backend`, minus the backends that cannot watch.

## Commands

The command tree (`cli.rs`, `Command`) is small:

- `watch`: the core loop described above; `-m/--mailbox` overrides the account's `mailbox` (default `INBOX`).
- `check`: opens each backend allowed by `--backend` and reports per-backend connectivity, so credential/network errors surface before a real `watch`. Mirrors `himalaya account check`.
- `manuals`, `completions`: man pages and shell completions.

Output follows the Pimalaya stdout/stderr rule: data and errors go to stdout through `pimalaya_cli::printer` (with `--json` switching to JSON), stderr carries logs only. Each subcommand is a clap-derived struct with an `execute(self, printer, config_paths, account, backend)` method; `cli.rs` is the single dispatch point.

## Configuration

Config is loaded by pimalaya-config from the first existing canonical path (or the `-c` / `MIRADOR_CONFIG` override), with later paths deep-merged on top. The schema (`config.rs`) is multi-account: named `[accounts.<name>]` blocks, each with optional `imap` / `jmap` / `maildir` sub-blocks keyed by protocol. Crucially `AccountConfig` does **not** set `deny_unknown_fields`: the same TOML file is shared with himalaya CLI v2 and himalaya-tui, so their extra keys (`smtp`, `m2dir`, `display-name`, …) coexist silently with the mirador-only ones (`mailbox`, the `hooks.on-*` tables). Mirador has no interactive wizard; `Config::load` bails with a pointer to `config.sample.toml` when no file resolves.

## Module layout

```
src/
  main.rs      entry point: parse Cli, build printer, dispatch
  cli.rs       Cli/Command, global flags (account, backend, json, log), execute dispatch
  backend.rs   Backend enum (auto/imap/jmap/maildir) + allow rules
  config.rs    TOML schema: Config, AccountConfig, per-backend blocks, hooks, SASL/TLS
  client.rs    open(): register one backend onto an EmailClientStd for watching
  watch.rs     the watch command: blocking watch_mailbox on a worker thread + event loop
  check.rs     the check command: per-backend connectivity report
  hook.rs      hook runner: notify-rust notifications + shell-command spawning
```
