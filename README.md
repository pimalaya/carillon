# 🔭 Mirador [![Matrix](https://img.shields.io/badge/chat-%23pimalaya-blue?style=flat&logo=matrix&logoColor=white)](https://matrix.to/#/#pimalaya:matrix.org) [![Mastodon](https://img.shields.io/badge/news-%40pimalaya-blue?style=flat&logo=mastodon&logoColor=white)](https://fosstodon.org/@pimalaya) [![Sponsor](https://img.shields.io/badge/sponsor-pink?style=flat&logo=github-sponsors&logoColor=white)](https://pimalaya.org/sponsor/)

CLI to watch mailbox changes, written in Rust

> [!CAUTION]
> Mirador is in active development and currently shipped as `v0.1.x`. Expect breaking changes between releases until stabilization. See the [migration guide](./MIGRATION.md) if you ran a pre-v0.1.0 build.

## Table of contents

- [Features](#features)
- [Coverage](#coverage)
- [Installation](#installation)
- [Configuration](#configuration)
- [Usage](#usage)
- [Interfaces](#interfaces)
- [AI policy](https://github.com/pimalaya/.github/blob/master/AI_POLICY.md)
- [License](#license)
- [Social](#social)
- [Contributing](./CONTRIBUTING.md)
- [Sponsoring](#sponsoring)

## Features

- **Four backends**: IMAP idles, JMAP polls for changes, Maildir re-lists, WebDAV asks what moved.
- **Not only mail**: a WebDAV collection is a CalDAV calendar or a CardDAV addressbook just as readily.
- **Every account at once**: one thread each, or `-a/--account` for a single one.
- **Self-healing**: a dropped watch reopens with a capped backoff, reading the credential again each time.
- **Five events**: an item added, removed or changed, flags added or removed. Flag names are the same on every backend, so a filter written once fires everywhere.
- **Hooks**: a desktop notification, a shell command, or both, with the event's fields as placeholders.
- **Shared configuration**: the same `[accounts.<name>]` block backs `himalaya` and `himalaya-tui`.
- **Authentication**: SASL for IMAP, basic and bearer for JMAP and WebDAV, secrets read from your own password manager.
- **JSON output**: `--json` on every data command, for scripts.
- **TLS**: [rustls](https://crates.io/crates/rustls) with ring (`rustls-ring`, default) or aws (`rustls-aws`) crypto, or [native-tls](https://crates.io/crates/native-tls) (`native-tls`).

> [!TIP]
> Mirador is written in [Rust](https://www.rust-lang.org/) and uses [cargo features](https://doc.rust-lang.org/cargo/reference/features.html) to gate backend support. The default feature set is declared in [Cargo.toml](./Cargo.toml).

## Coverage

| Spec      | What is covered |
|-----------|-----------------|
| [2177]    | IMAP idle: the held connection a watch waits on, woken by the server on every change |
| [7162]    | Quick mailbox resynchronization: server-named deltas where the server offers them, a local re-read and diff where it does not |
| [4959]    | SASL initial response, forced on or off for providers that advertise it and then refuse it |
| [2971]    | IMAP identification, required by some providers right after authentication |
| [4616]    | SASL plain, the password mechanism |
| [4505]    | SASL anonymous, for servers accepting unauthenticated sessions |
| [7677]    | SASL scram-sha-256, the challenge-response mechanism |
| [7628]    | SASL oauthbearer, an OAuth 2.0 token issued by an external broker |
| [8620]    | The JMAP core: session discovery, the changes and get method shapes, request batching |
| [8621]    | JMAP for mail: mailboxes, emails, and the change stream a watch polls |
| [maildir] | The original Maildir layout, plus the Maildir++ subfolder convention |
| [4918]    | WebDAV itself: the report a collection answers, and the etag that says an item moved |
| [6578]    | Collection synchronization: the token a poll carries, what changed since it, and what to do when the server refuses it |

[2177]: https://www.rfc-editor.org/rfc/rfc2177
[7162]: https://www.rfc-editor.org/rfc/rfc7162
[4959]: https://www.rfc-editor.org/rfc/rfc4959
[2971]: https://www.rfc-editor.org/rfc/rfc2971
[4616]: https://www.rfc-editor.org/rfc/rfc4616
[4505]: https://www.rfc-editor.org/rfc/rfc4505
[7677]: https://www.rfc-editor.org/rfc/rfc7677
[7628]: https://www.rfc-editor.org/rfc/rfc7628
[8620]: https://www.rfc-editor.org/rfc/rfc8620
[8621]: https://www.rfc-editor.org/rfc/rfc8621
[maildir]: https://cr.yp.to/proto/maildir.html
[4918]: https://www.rfc-editor.org/rfc/rfc4918
[6578]: https://www.rfc-editor.org/rfc/rfc6578

## Installation

### Pre-built binary

Mirador is not released yet. Until it is, check out the [releases](https://github.com/pimalaya/mirador/actions/workflows/releases.yml) GitHub workflow and look for the *Artifacts* section. These pre-built binaries are built from the master branch.

> [!NOTE]
> Such binaries are built with the default cargo features. If you need specific features, please use another installation method.

### Cargo

```sh
cargo install --locked --git https://github.com/pimalaya/mirador.git
```

With IMAP support only, which drops the JMAP, Maildir and WebDAV backends:

```sh
cargo install --locked --git https://github.com/pimalaya/mirador.git \
  --no-default-features \
  --features imap,rustls-ring
```

### Nix

If you have the [Flakes](https://nixos.wiki/wiki/Flakes) feature enabled:

```sh
nix profile install github:pimalaya/mirador
```

Or run without installing:

```sh
nix run github:pimalaya/mirador
```

### Sources

```sh
git clone https://github.com/pimalaya/mirador
cd mirador
nix run
```

## Configuration

Mirador ships no wizard: copy the annotated [config.sample.toml](./config.sample.toml), keep the account and the hooks you want, and delete the rest.

A configuration is loaded from the first valid path among $XDG_CONFIG_HOME/mirador/config.toml, $HOME/.config/mirador/config.toml and $HOME/.miradorrc. Override it with `-c <PATH>` or `MIRADOR_CONFIG=<PATH>`, `:`-separated to deep-merge several files on top of the first.

[himalaya](https://github.com/pimalaya/himalaya) and [himalaya-tui](https://github.com/pimalaya/himalaya-tui) read those same paths, so one file backs all three binaries. Each backend is a protocol key (`imap`, `jmap`, `maildir`, `dav`) written as dotted entries under `[accounts.<name>]`, and the keys only mirador reads sit next to the shared ones, ignored by the others.

> [!WARNING]
> A pre-v0.1.0 configuration is not compatible with `v0.1.0`, since the schema differs. Read the [migration guide](./MIGRATION.md) first, or rewrite the file from [config.sample.toml](./config.sample.toml).

### Backend selection

A watch opens exactly one connection, so an account declaring several backends picks one at startup: `-b/--backend {imap,jmap,maildir,dav}` pins it and fails when the block is absent, while `-b auto`, the default, takes the first configured among IMAP, JMAP, Maildir, WebDAV.

### Hooks

Every event has its own hook under the `hooks.` namespace: `hooks.on-item-added`, `hooks.on-item-removed`, `hooks.on-item-changed`, `hooks.on-flags-added` and `hooks.on-flags-removed`. The earlier `on-message-*` names still work. Each fires a notification, a command, or both, and flag hooks take an optional `flags` filter. A hook that fails is logged and never stops the watch. The shapes and the full placeholder list are documented in [config.sample.toml](./config.sample.toml).

## Usage

Watch every configured account, until interrupted:

```sh
mirador watch
```

Watch one account, on a mailbox other than the one it configures:

```sh
mirador -a work watch -m Drafts
```

Check that an account still connects and authenticates, on every backend it declares, before trusting a watch to keep running:

```sh
mirador -a work check
```

Force a backend on an account declaring several:

```sh
mirador -b jmap watch
```

Every command and every flag is documented behind `--help`. Man pages and shell completions are generated by `mirador manuals <DIR>` and `mirador completions <DIR>`.

Logs go to stderr, so they can be redirected to a file while the command output stays on stdout:

```sh
mirador watch --log-level debug 2>/tmp/mirador.log
```

Use `--log-file <PATH>` to append them to a file directly. When `--log-level` is omitted the `RUST_LOG` environment variable is consulted, and `RUST_BACKTRACE=1` adds the full error backtrace.

## Interfaces

Mirador is one of several front-ends to the Pimalaya libraries. See [pimalaya/himalaya#interfaces](https://github.com/pimalaya/himalaya#interfaces) for the full list.

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
