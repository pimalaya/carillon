# 🔭 Mirador [![Matrix](https://img.shields.io/badge/chat-%23pimalaya-blue?style=flat&logo=matrix&logoColor=white)](https://matrix.to/#/#pimalaya:matrix.org) [![Mastodon](https://img.shields.io/badge/news-%40pimalaya-blue?style=flat&logo=mastodon&logoColor=white)](https://fosstodon.org/@pimalaya)

CLI to watch mailbox changes, written in Rust

![screenshot](./screenshot.jpeg)

> [!CAUTION]
> Mirador v2 is a full rewrite on top of the Pimalaya `io-*` stack. The `v2.0.0-rc` series is being smoke-tested against real accounts; expect breaking changes between release candidates. See [MIGRATION.md](./MIGRATION.md) when coming from v1.

## Table of contents

- [Features](#features)
- [Installation](#installation)
  - [Pre-built binary](#pre-built-binary)
  - [Cargo](#cargo)
  - [Nix](#nix)
  - [Sources](#sources)
- [Configuration](#configuration)
  - [Backend selection](#backend-selection)
  - [Hooks](#hooks)
- [Usage](#usage)
- [Migration](#migration)
- [Interfaces](#interfaces)
- [AI disclosure](#ai-disclosure)
- [Social](#social)
- [Sponsoring](#sponsoring)

## Features

- Remote backends: **IMAP** via [RFC 2177 IDLE](https://datatracker.ietf.org/doc/html/rfc2177), **JMAP** via [RFC 8620 §7.2 EventSource](https://datatracker.ietf.org/doc/html/rfc8620#section-7.2) push
- Local backend: **Maildir** <sup>[specs](https://cr.yp.to/proto/maildir.html)</sup> via filesystem notifications
- Watch events: **`on-message-added`**, **`on-message-removed`**, **`on-flags-added`**, **`on-flags-removed`** (flag hooks accept an optional `flags = [...]` filter)
- Hook actions: **system notification** via [notify-rust](https://crates.io/crates/notify-rust) and **shell command** via `sh -c`
- Shell-style placeholders (`$name` / `${name}`) in hook strings: `id`, `mailbox`, `subject`, `sender`, `sender_name`, `sender_address`, `recipient`, `recipient_name`, `recipient_address`, `flag`, `flags`. Shell-command hooks receive them as environment variables, so the shell's own expansion does the substitution: write `"$subject"` (quoted) for safe whitespace handling.
- **Simple auth** support for IMAP: anonymous, login, plain, oauthbearer, xoauth2, scram-sha-256
- **HTTP auth** support for JMAP: basic, bearer
- **TLS** support:
  - [Rustls](https://crates.io/crates/rustls) with ring crypto
  - [Rustls](https://crates.io/crates/rustls) with aws crypto (requires `rustls-aws` feature)
  - [Native TLS](https://crates.io/crates/native-tls) (requires `native-tls` feature)
- **Shared configuration file** with `himalaya` and `himalaya-tui`: the same `[accounts.<name>]` block loads on all three binaries (see [Configuration](#configuration))

> [!TIP]
> Mirador is written in [Rust](https://www.rust-lang.org/) and uses [cargo features](https://doc.rust-lang.org/cargo/reference/features.html) to gate backend support. The default feature set is declared in [Cargo.toml](./Cargo.toml).

## Installation

### Pre-built binary

Mirador `v2` is not yet released; the only way to get a pre-built binary today is to check out the [releases](https://github.com/pimalaya/mirador/actions/workflows/releases.yml) GitHub workflow and look for the *Artifacts* section.

> [!NOTE]
> Such binaries are built with the default cargo features. If you need specific features, please use another installation method.

### Cargo

```
cargo install --locked --git https://github.com/pimalaya/mirador.git
```

With only IMAP support:

```
cargo install --locked --git https://github.com/pimalaya/mirador.git \
  --no-default-features \
  --features imap,rustls-ring
```

### Nix

If you have the [Flakes](https://nixos.wiki/wiki/Flakes) feature enabled:

```
nix profile install github:pimalaya/mirador
```

Or run without installing:

```
nix run github:pimalaya/mirador
```

### Sources

```
git clone https://github.com/pimalaya/mirador
cd mirador
nix run
```

## Configuration

Copy [config.sample.toml](./config.sample.toml) to `$XDG_CONFIG_HOME/mirador/config.toml` and edit it. Mirador does not ship an interactive wizard; the sample documents every backend block and hook with inline comments.

A persistent configuration is loaded from the first valid path among:

- `$XDG_CONFIG_HOME/mirador/config.toml`
- `$HOME/.config/mirador/config.toml`
- `$HOME/.miradorrc`

These are the same paths the [himalaya](https://github.com/pimalaya/himalaya) CLI and [himalaya-tui](https://github.com/pimalaya/himalaya-tui) look at: one TOML file backs all three binaries, **starting from himalaya CLI v2**. Each backend lives under its own protocol key (`imap.*`, `jmap.*`, `maildir.*`), declared as flat dotted entries under `[accounts.<name>]`. Mirador-only fields (`folder`, the four `on-*` hook tables) coexist with the shared keys and are silently ignored by the other binaries.

> [!WARNING]
> A mirador `v1` configuration file is **not** compatible with `v2`: the schema differs. See [MIGRATION.md](./MIGRATION.md) (or rewrite the file using [config.sample.toml](./config.sample.toml) as a template) before pointing `v2` at it.

Override the path with `-c <PATH>` or `MIRADOR_CONFIG=<PATH>`; multiple paths can be passed at once, separated by `:`. The first one is the base and the rest are deep-merged on top.

### Backend selection

An account may declare more than one of the `imap`, `jmap`, `maildir` blocks (so the same TOML file can drive `mirador` and `himalaya` against different backends). Mirador opens exactly one connection per `watch`, so the active backend is picked at startup:

- `-b/--backend imap | jmap | maildir` pins the active backend; the command bails when the account has no matching block.
- `-b auto` (default) picks the first configured block in this order: IMAP, then JMAP, then Maildir.

### Hooks

Mirador fires zero or more hooks per [watch event kind](#features). Each hook config can declare a system notification, a shell command, or both:

```toml
[accounts.example.on-message-added]
notify = { summary = "New mail from $sender", body = "$subject" }
cmd = "mbsync example"

# Flag hooks may filter on the IANA flag name (case-insensitive, with or
# without the leading "\" / "$"):
[accounts.example.on-flags-added]
flags = ["Seen"]
cmd = 'echo "$id marked read" >> ~/.local/state/mirador.log'
```

Notifications use [notify-rust](https://crates.io/crates/notify-rust) (D-Bus / `NSUserNotification` / Windows toast). Shell commands run via `sh -c`; failures are logged at `warn` so a broken script never crashes the watcher.

## Usage

```
mirador watch                  # watch the default account's folder
mirador -a work watch          # watch the `work` account
mirador watch -f Drafts        # watch a specific folder
mirador -b jmap watch          # force JMAP when several backends are configured
mirador check                  # validate the account against each configured backend
mirador completions bash ./out # generate shell completions
mirador manuals ./out          # generate man pages
```

The watch loop runs until `Ctrl+C`; the IMAP / JMAP / Maildir driver winds down cleanly (sends `IDLE DONE`, closes the SSE socket, drops the notify watcher) before the binary exits.

## Migration

Coming from `v1.x`? Read [MIGRATION.md](./MIGRATION.md). The v2 configuration schema is incompatible with v1: the `[accounts.<name>.backend]` table is gone, replaced by parallel `imap.*` / `jmap.*` / `maildir.*` dotted keys aligned with himalaya CLI v2.

## Interfaces

Mirador is one of several front-ends to the Pimalaya libraries. See [pimalaya/himalaya#interfaces](https://github.com/pimalaya/himalaya#interfaces) for the full list (CLI, TUI, Vim, Emacs, Raycast).

## AI disclosure

This project is developed with AI assistance. This section documents how, so users and downstream packagers can make informed decisions.

- **Tools**: Claude Code (Anthropic), Opus 4.7, invoked locally with a persistent project-scoped memory and a small set of repo-specific rules.

- **Used for**: Refactors, mechanical multi-file edits, boilerplate (feature gates, error enums, derive macros, trait impls), test scaffolding, doc polish, exploratory design conversations.

- **Not used for**: Engineering, critical code, git manipulation (commit, merge, rebase…), real-world tests.

- **Verification**: Every AI-assisted change is read, compiled, tested, and formatted before commit (`nix develop --command cargo check / cargo test / cargo fmt`). Behavioural correctness is verified against the relevant RFC or upstream spec, not assumed from the model output. Tests are never adjusted to fit AI-generated code; the code is adjusted to fit correct behaviour.

- **Limitations**: AI models occasionally produce code that compiles and passes tests but is subtly wrong: off-by-one errors, missed edge cases, plausible but nonexistent APIs, stale RFC references. The verification workflow catches most of this; it does not catch all of it. Bug reports are welcome and taken seriously.

- **Last reviewed**: 30/05/2026

## Social

- Chat on [Matrix](https://matrix.to/#/#pimalaya:matrix.org)
- News on [Mastodon](https://fosstodon.org/@pimalaya) or [RSS](https://fosstodon.org/@pimalaya.rss)
- Mail at [pimalaya.org@posteo.net](mailto:pimalaya.org@posteo.net)

## Sponsoring

[![nlnet](https://nlnet.nl/logo/banner-160x60.png)](https://nlnet.nl/)

Special thanks to the [NLnet foundation](https://nlnet.nl/) and the [European Commission](https://www.ngi.eu/) that have been financially supporting the project for years:

- 2022 → 2023: [NGI Assure](https://nlnet.nl/project/Himalaya/)
- 2023 → 2024: [NGI Zero Entrust](https://nlnet.nl/project/Pimalaya/)
- 2024 → 2026: [NGI Zero Core](https://nlnet.nl/project/Pimalaya-PIM/)
- *2027 in preparation…*

If you appreciate the project, feel free to donate using one of the following providers:

[![GitHub](https://img.shields.io/badge/-GitHub%20Sponsors-fafbfc?logo=GitHub%20Sponsors)](https://github.com/sponsors/soywod)
[![Ko-fi](https://img.shields.io/badge/-Ko--fi-ff5e5a?logo=Ko-fi&logoColor=ffffff)](https://ko-fi.com/soywod)
[![Buy Me a Coffee](https://img.shields.io/badge/-Buy%20Me%20a%20Coffee-ffdd00?logo=Buy%20Me%20A%20Coffee&logoColor=000000)](https://www.buymeacoffee.com/soywod)
[![Liberapay](https://img.shields.io/badge/-Liberapay-f6c915?logo=Liberapay&logoColor=222222)](https://liberapay.com/soywod)
[![thanks.dev](https://img.shields.io/badge/-thanks.dev-000000?logo=data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjQuMDk3IiBoZWlnaHQ9IjE3LjU5NyIgY2xhc3M9InctMzYgbWwtMiBsZzpteC0wIHByaW50Om14LTAgcHJpbnQ6aW52ZXJ0IiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxwYXRoIGQ9Ik05Ljc4MyAxNy41OTdINy4zOThjLTEuMTY4IDAtMi4wOTItLjI5Ny0yLjc3My0uODktLjY4LS41OTMtMS4wMi0xLjQ2Mi0xLjAyLTIuNjA2di0xLjM0NmMwLTEuMDE4LS4yMjctMS43NS0uNjc4LTIuMTk1LS40NTItLjQ0Ni0xLjIzMi0uNjY5LTIuMzQtLjY2OUgwVjcuNzA1aC41ODdjMS4xMDggMCAxLjg4OC0uMjIyIDIuMzQtLjY2OC40NTEtLjQ0Ni42NzctMS4xNzcuNjc3LTIuMTk1VjMuNDk2YzAtMS4xNDQuMzQtMi4wMTMgMS4wMjEtMi42MDZDNS4zMDUuMjk3IDYuMjMgMCA3LjM5OCAwaDIuMzg1djEuOTg3aC0uOTg1Yy0uMzYxIDAtLjY4OC4wMjctLjk4LjA4MmExLjcxOSAxLjcxOSAwIDAgMC0uNzM2LjMwN2MtLjIwNS4xNTYtLjM1OC4zODQtLjQ2LjY4Mi0uMTAzLjI5OC0uMTU0LjY4Mi0uMTU0IDEuMTUxVjUuMjNjMCAuODY3LS4yNDkgMS41ODYtLjc0NSAyLjE1NS0uNDk3LjU2OS0xLjE1OCAxLjAwNC0xLjk4MyAxLjMwNXYuMjE3Yy44MjUuMyAxLjQ4Ni43MzYgMS45ODMgMS4zMDUuNDk2LjU3Ljc0NSAxLjI4Ny43NDUgMi4xNTR2MS4wMjFjMCAuNDcuMDUxLjg1NC4xNTMgMS4xNTIuMTAzLjI5OC4yNTYuNTI1LjQ2MS42ODIuMTkzLjE1Ny40MzcuMjYuNzMyLjMxMi4yOTUuMDUuNjIzLjA3Ni45ODQuMDc2aC45ODVabTE0LjMxNC03LjcwNmgtLjU4OGMtMS4xMDggMC0xLjg4OC4yMjMtMi4zNC42NjktLjQ1LjQ0NS0uNjc3IDEuMTc3LS42NzcgMi4xOTVWMTQuMWMwIDEuMTQ0LS4zNCAyLjAxMy0xLjAyIDIuNjA2LS42OC41OTMtMS42MDUuODktMi43NzQuODloLTIuMzg0di0xLjk4OGguOTg0Yy4zNjIgMCAuNjg4LS4wMjcuOTgtLjA4LjI5Mi0uMDU1LjUzOC0uMTU3LjczNy0uMzA4LjIwNC0uMTU3LjM1OC0uMzg0LjQ2LS42ODIuMTAzLS4yOTguMTU0LS42ODIuMTU0LTEuMTUydi0xLjAyYzAtLjg2OC4yNDgtMS41ODYuNzQ1LTIuMTU1LjQ5Ny0uNTcgMS4xNTgtMS4wMDQgMS45ODMtMS4zMDV2LS4yMTdjLS44MjUtLjMwMS0xLjQ4Ni0uNzM2LTEuOTgzLTEuMzA1LS40OTctLjU3LS43NDUtMS4yODgtLjc0NS0yLjE1NXYtMS4wMmMwLS40Ny0uMDUxLS44NTQtLjE1NC0xLjE1Mi0uMTAyLS4yOTgtLjI1Ni0uNTI2LS40Ni0uNjgyYTEuNzE5IDEuNzE5IDAgMCAwLS43MzctLjMwNyA1LjM5NSA1LjM5NSAwIDAgMC0uOTgtLjA4MmgtLjk4NFYwaDIuMzg0YzEuMTY5IDAgMi4wOTMuMjk3IDIuNzc0Ljg5LjY4LjU5MyAxLjAyIDEuNDYyIDEuMDIgMi42MDZ2MS4zNDZjMCAxLjAxOC4yMjYgMS43NS42NzggMi4xOTUuNDUxLjQ0NiAxLjIzMS42NjggMi4zNC42NjhoLjU4N3oiIGZpbGw9IiNmZmYiLz48L3N2Zz4=)](https://thanks.dev/soywod)
[![PayPal](https://img.shields.io/badge/-PayPal-0079c1?logo=PayPal&logoColor=ffffff)](https://www.paypal.com/paypalme/soywod)
