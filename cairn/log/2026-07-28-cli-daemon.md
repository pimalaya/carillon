---
cairn: log
change: cli-daemon
landed: 2026-07-28
---

# Build the carillon CLI daemon on carillon-core

Turned the empty stub (a hello-world main) into a working daemon that hosts carillon-core. It reads a TOML config of named IMAP watches and, per watch, runs a supervisor task that owns the transport and reconnect and drives core's one-session `imap::watch` over the opened TLS stream. Every content-free ring is sent on a shared channel and routed by a dispatch task to that watch's consumers. Adopted Cairn from the start: the cairn root, the activation surface, and the daemon capability spec.

## What landed

Five flat modules. config parses the TOML (a table of named watches, each with host, port, login, mailbox, a password or password_command, and the notify and exec consumer toggles). transport opens TCP + TLS with `ClientConfig::with_platform_verifier`, no SSRF guard since the daemon trusts the user's own config. supervisor is the reconnect loop: resolve a fresh credential, connect, drive core's watch, then back off with jitter and reconnect, until shutdown. consumer holds the two reactions, notify (a content-free desktop notification via notify-rust, run on a blocking thread) and exec (a shell command with the ring's fields in CARILLON_* environment variables, spawned directly via tokio, not the deprecated io-process). main wires it together: load config, build the TLS connector, spawn one supervisor per watch and a dispatch task, and abort on ctrl-c.

This validates the split end to end from the frontend side: the frontend opens the transport and owns reconnect, core drives the conversation over the stream, and the credential is resolved fresh per attempt (minimal residency). The `carillon-core` decision to be transport-agnostic paid off: the CLI pulls in tokio-rustls and the platform verifier, and core stays clean.

## Guidelines

Applied from the start. Binary manifest per cargo-009 (release profile, no docs.rs block). main.rs opens directly with the architecture header (header-002). Every pub item documented for clap help (inline-002, inline-006). Imports merged per crate-004 and ordered std then third-party then crate. Logging via the log crate: info on actions, warn on consumer or connection failures, debug for loop internals (logging-004). Green on check, clippy, and fmt.

## Deferred

OAuth credentials (the CLI resolves only password and password_command for now). The poll transport class (CardDAV) arrives with core layer 2b. Alignment onto the pimalaya-cli toolkit (printer, logger, wizard) is deferred: a daemon with a single config flag needs little of it, and clap plus env_logger keep the surface small.
