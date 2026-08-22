# 🔭 Mirador [![Matrix](https://img.shields.io/badge/chat-%23pimalaya-blue?style=flat&logo=matrix&logoColor=white)](https://matrix.to/#/#pimalaya:matrix.org) [![Mastodon](https://img.shields.io/badge/news-%40pimalaya-blue?style=flat&logo=mastodon&logoColor=white)](https://fosstodon.org/@pimalaya) [![Sponsor](https://img.shields.io/badge/sponsor-pink?style=flat&logo=github-sponsors&logoColor=white)](https://pimalaya.org/sponsor/)

CLI to watch mailbox changes, written in Rust

![screenshot](./screenshot.jpeg)

> [!CAUTION]
> Mirador is in active development and currently shipped as `v0.1.x`. Expect breaking changes between releases until stabilization. See [MIGRATION.md](./MIGRATION.md) if you ran a pre-v0.1.0 build.

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
- [License](#license)
- [AI policy](https://github.com/pimalaya/.github/blob/master/AI_POLICY.md)
- [Social](#social)
- [Contributing](https://github.com/pimalaya/.github/blob/master/CONTRIBUTING.md)
- [Sponsoring](#sponsoring)

## Features

- Remote backends: **IMAP** via [RFC 2177 IDLE](https://datatracker.ietf.org/doc/html/rfc2177), **JMAP** via [RFC 8620 §7.2 EventSource](https://datatracker.ietf.org/doc/html/rfc8620#section-7.2) push
- Local backend: **Maildir** <sup>[specs](https://cr.yp.to/proto/maildir.html)</sup> via filesystem notifications
- Watch events: **`on-message-added`**, **`on-message-removed`**, **`on-flags-added`**, **`on-flags-removed`** (flag hooks accept an optional `flags = [...]` filter)
- Hook actions: **system notification** via [notify-rust](https://crates.io/crates/notify-rust) and **shell command** (TOML string handed to `/bin/sh -c` on Unix / `cmd /C` on Windows; or a TOML `[program, args…]` list spawned directly with no shell)
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

Mirador is not yet released; the only way to get a pre-built binary today is to check out the [releases](https://github.com/pimalaya/mirador/actions/workflows/releases.yml) GitHub workflow and look for the *Artifacts* section.

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

These are the same paths the [himalaya](https://github.com/pimalaya/himalaya) CLI and [himalaya-tui](https://github.com/pimalaya/himalaya-tui) look at: one TOML file backs all three binaries, **starting from himalaya CLI v2**. Each backend lives under its own protocol key (`imap.*`, `jmap.*`, `maildir.*`), declared as flat dotted entries under `[accounts.<name>]`. Mirador-only fields (`mailbox`, the `hooks.on-*` tables) coexist with the shared keys and are silently ignored by the other binaries.

> [!WARNING]
> A pre-v0.1.0 mirador configuration file is **not** compatible with `v0.1.0`: the schema differs. See [MIGRATION.md](./MIGRATION.md) (or rewrite the file using [config.sample.toml](./config.sample.toml) as a template) before pointing `v0.1.0` at it.

Override the path with `-c <PATH>` or `MIRADOR_CONFIG=<PATH>`; multiple paths can be passed at once, separated by `:`. The first one is the base and the rest are deep-merged on top.

### Backend selection

An account may declare more than one of the `imap`, `jmap`, `maildir` blocks (so the same TOML file can drive `mirador` and `himalaya` against different backends). Mirador opens exactly one connection per `watch`, so the active backend is picked at startup:

- `-b/--backend imap | jmap | maildir` pins the active backend; the command bails when the account has no matching block.
- `-b auto` (default) picks the first configured block in this order: IMAP, then JMAP, then Maildir.

### Hooks

Mirador fires zero or more hooks per [watch event kind](#features). Each hook config can declare a system notification, a shell command, or both:

```toml
# String `cmd`: handed to /bin/sh -c on Unix, cmd /C on Windows.
# Placeholders are env vars; the shell does the expansion.
hooks.on-message-added.notify = { summary = "New mail from $sender", body = "$subject" }
hooks.on-message-added.cmd = 'echo "$id arrived" >> ~/.local/state/mirador.log'

# List `cmd`: [program, args...] spawned directly. Flag hooks accept
# an optional `flags = [...]` filter (IANA flag name, case-insensitive).
hooks.on-flags-added.flags = ["Seen"]
hooks.on-flags-added.cmd = ["notify-send", "New flag on $id"]
```

Both `cmd` shapes are decoded by [`pimalaya_config::command`](https://github.com/pimalaya/config). Notifications use [notify-rust](https://crates.io/crates/notify-rust) (D-Bus / `NSUserNotification` / Windows toast). Failures are logged at `warn` so a broken hook never crashes the watcher.

## Usage

```
mirador watch                  # watch the default account's mailbox
mirador -a work watch          # watch the `work` account
mirador watch -m Drafts        # watch a specific mailbox
mirador -b jmap watch          # force JMAP when several backends are configured
mirador check                  # validate the account against each configured backend
mirador completions bash ./out # generate shell completions
mirador manuals ./out          # generate man pages
```

The watch loop runs until `Ctrl+C`; the IMAP / JMAP / Maildir driver winds down cleanly (sends `IDLE DONE`, closes the SSE socket, drops the notify watcher) before the binary exits.

## Migration

Coming from a pre-v0.1.0 (draft) mirador build? Read [MIGRATION.md](./MIGRATION.md). The `v0.1.0` configuration schema is incompatible with the earlier `[accounts.<name>.backend]` shape, which is now replaced by parallel `imap.*` / `jmap.*` / `maildir.*` dotted keys aligned with himalaya CLI v2.

## Interfaces

Mirador is one of several front-ends to the Pimalaya libraries. See [pimalaya/himalaya#interfaces](https://github.com/pimalaya/himalaya#interfaces) for the full list (CLI, TUI, Vim, Emacs, Raycast).

## License

This project is licensed under either of:

- [MIT license](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

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
- 2026 → 2027: [NGI Zero Commons Fund](https://nlnet.nl/project/Pimalaya-pimdir/)

This program is part of Pimalaya, free software funded entirely by grants and donations. If you find it useful, consider [sponsoring](https://pimalaya.org/sponsor/) its development:

[![GitHub](https://img.shields.io/badge/-GitHub%20Sponsors-fafbfc?logo=GitHub%20Sponsors)](https://github.com/sponsors/soywod)
[![Ko-fi](https://img.shields.io/badge/-Ko--fi-ff5e5a?logo=Ko-fi&logoColor=ffffff)](https://ko-fi.com/pimalaya)
[![Buy Me a Coffee](https://img.shields.io/badge/-Buy%20Me%20a%20Coffee-ffdd00?logo=Buy%20Me%20A%20Coffee&logoColor=000000)](https://www.buymeacoffee.com/pimalaya)
[![Liberapay](https://img.shields.io/badge/-Liberapay-f6c915?logo=Liberapay&logoColor=222222)](https://liberapay.com/pimalaya)
[![thanks.dev](https://img.shields.io/badge/-thanks.dev-000000?logo=data:image/svg+xml;base64,PHN2ZyB3aWR0aD0iMjQuMDk3IiBoZWlnaHQ9IjE3LjU5NyIgY2xhc3M9InctMzYgbWwtMiBsZzpteC0wIHByaW50Om14LTAgcHJpbnQ6aW52ZXJ0IiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPjxwYXRoIGQ9Ik05Ljc4MyAxNy41OTdINy4zOThjLTEuMTY4IDAtMi4wOTItLjI5Ny0yLjc3My0uODktLjY4LS41OTMtMS4wMi0xLjQ2Mi0xLjAyLTIuNjA2di0xLjM0NmMwLTEuMDE4LS4yMjctMS43NS0uNjc4LTIuMTk1LS40NTItLjQ0Ni0xLjIzMi0uNjY5LTIuMzQtLjY2OUgwVjcuNzA1aC41ODdjMS4xMDggMCAxLjg4OC0uMjIyIDIuMzQtLjY2OC40NTEtLjQ0Ni42NzctMS4xNzcuNjc3LTIuMTk1VjMuNDk2YzAtMS4xNDQuMzQtMi4wMTMgMS4wMjEtMi42MDZDNS4zMDUuMjk3IDYuMjMgMCA3LjM5OCAwaDIuMzg1djEuOTg3aC0uOTg1Yy0uMzYxIDAtLjY4OC4wMjctLjk4LjA4MmExLjcxOSAxLjcxOSAwIDAgMC0uNzM2LjMwN2MtLjIwNS4xNTYtLjM1OC4zODQtLjQ2LjY4Mi0uMTAzLjI5OC0uMTU0LjY4Mi0uMTU0IDEuMTUxVjUuMjNjMCAuODY3LS4yNDkgMS41ODYtLjc0NSAyLjE1NS0uNDk3LjU2OS0xLjE1OCAxLjAwNC0xLjk4MyAxLjMwNXYuMjE3Yy44MjUuMyAxLjQ4Ni43MzYgMS45ODMgMS4zMDUuNDk2LjU3Ljc0NSAxLjI4Ny43NDUgMi4xNTR2MS4wMjFjMCAuNDcuMDUxLjg1NC4xNTMgMS4xNTIuMTAzLjI5OC4yNTYuNTI1LjQ2MS42ODIuMTkzLjE1Ny40MzcuMjYuNzMyLjMxMi4yOTUuMDUuNjIzLjA3Ni45ODQuMDc2aC45ODVabTE0LjMxNC03LjcwNmgtLjU4OGMtMS4xMDggMC0xLjg4OC4yMjMtMi4zNC42NjktLjQ1LjQ0NS0uNjc3IDEuMTc3LS42NzcgMi4xOTVWMTQuMWMwIDEuMTQ0LS4zNCAyLjAxMy0xLjAyIDIuNjA2LS42OC41OTMtMS42MDUuODktMi43NzQuODloLTIuMzg0di0xLjk4OGguOTg0Yy4zNjIgMCAuNjg4LS4wMjcuOTgtLjA4LjI5Mi0uMDU1LjUzOC0uMTU3LjczNy0uMzA4LjIwNC0uMTU3LjM1OC0uMzg0LjQ2LS42ODIuMTAzLS4yOTguMTU0LS42ODIuMTU0LTEuMTUydi0xLjAyYzAtLjg2OC4yNDgtMS41ODYuNzQ1LTIuMTU1LjQ5Ny0uNTcgMS4xNTgtMS4wMDQgMS45ODMtMS4zMDV2LS4yMTdjLS44MjUtLjMwMS0xLjQ4Ni0uNzM2LTEuOTgzLTEuMzA1LS40OTctLjU3LS43NDUtMS4yODgtLjc0NS0yLjE1NXYtMS4wMmMwLS40Ny0uMDUxLS44NTQtLjE1NC0xLjE1Mi0uMTAyLS4yOTgtLjI1Ni0uNTI2LS40Ni0uNjgyYTEuNzE5IDEuNzE5IDAgMCAwLS43MzctLjMwNyA1LjM5NSA1LjM5NSAwIDAgMC0uOTgtLjA4MmgtLjk4NFYwaDIuMzg0YzEuMTY5IDAgMi4wOTMuMjk3IDIuNzc0Ljg5LjY4LjU5MyAxLjAyIDEuNDYyIDEuMDIgMi42MDZ2MS4zNDZjMCAxLjAxOC4yMjYgMS43NS42NzggMi4xOTUuNDUxLjQ0NiAxLjIzMS42NjggMi4zNC42NjhoLjU4N3oiIGZpbGw9IiNmZmYiLz48L3N2Zz4=)](https://thanks.dev/u/gh/soywod)
[![PayPal](https://img.shields.io/badge/-PayPal-0079c1?logo=PayPal&logoColor=ffffff)](https://www.paypal.com/paypalme/soywod)
