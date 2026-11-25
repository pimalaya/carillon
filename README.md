# 🔔 Carillon [![Matrix](https://img.shields.io/badge/chat-%23pimalaya-blue?style=flat&logo=matrix&logoColor=white)](https://matrix.to/#/#pimalaya:matrix.org) [![Mastodon](https://img.shields.io/badge/news-%40pimalaya-blue?style=flat&logo=mastodon&logoColor=white)](https://fosstodon.org/@pimalaya) [![Sponsor](https://img.shields.io/badge/sponsor-pink?style=flat&logo=github-sponsors&logoColor=white)](https://pimalaya.org/sponsor/)

CLI to watch PIM collection changes

```sh
$ carillon watch
```

```text
[personal] watching mailbox `INBOX` over maildir, polling
[personal] mailbox `INBOX`: message 1795248000.1.laptop added
Watch stopped by a signal
```

## Table of contents

- [Features](#features)
- [Installation](#installation)
- [Configuration](#configuration)
  - [Google](#google)
  - [Microsoft](#microsoft)
  - [Fastmail](#fastmail)
- [Usage](#usage)
- [Alternatives](#alternatives)
- [AI policy](https://github.com/pimalaya/.github/blob/master/AI_POLICY.md)
- [License](#license)
- [Social](#social)
- [Contributing](./CONTRIBUTING.md)
- [Sponsoring](#sponsoring)

## Features

- **Mail, contacts and calendars** over IMAP, JMAP, Maildir, CalDAV, CardDAV, Microsoft Graph and the Google APIs
- **Push where offered**: IMAP IDLE and the JMAP event stream, a poll everywhere else
- **Hooks**: a command, a desktop notification or both on any change, the item given as variables
- **Alarms** of events and tasks fired on time, from a CalDAV calendar or the pimdir store Neverest syncs
- **Multi-account**: every account watched at once, each reconnecting on its own
- **Account check** proving every backend connects and authenticates
- **JSON output** of every event and result, described by `carillon json-schema`
- **TLS** via Rustls (`rustls-ring` default, `rustls-aws`) or Native TLS (`native-tls`), client certificates included

## Installation

<details>
  <summary>Debian, Ubuntu</summary>

  Carillon is not in the official repositories yet, but the signed APT repository of [pkg.pimalaya.org](https://pkg.pimalaya.org) ships the latest release with its man pages and shell completions. Add the repository once:

  ```sh
  sudo install -d -m 0755 /etc/apt/keyrings
  curl -fsSL https://pkg.pimalaya.org/pimalaya.asc | sudo tee /etc/apt/keyrings/pimalaya.asc >/dev/null
  sudo tee /etc/apt/sources.list.d/pimalaya.sources >/dev/null <<EOF
  Types: deb
  URIs: https://pkg.pimalaya.org/apt
  Suites: stable
  Components: main
  Signed-By: /etc/apt/keyrings/pimalaya.asc
  EOF
  sudo apt update
  ```

  Then install Carillon:

  ```sh
  sudo apt install carillon
  ```
</details>

<details>
  <summary>Fedora, RHEL, Rocky Linux, AlmaLinux</summary>

  Carillon is not in the official repositories yet, but the signed DNF repository of [pkg.pimalaya.org](https://pkg.pimalaya.org) ships the latest release with its man pages and shell completions:

  ```sh
  sudo curl -fsSL -o /etc/yum.repos.d/pimalaya.repo https://pkg.pimalaya.org/pimalaya.repo
  sudo dnf install carillon
  ```
</details>

<details>
  <summary>Homebrew (macOS, Linux)</summary>

  Carillon is not in homebrew-core yet, but the [pimalaya/tap](https://github.com/pimalaya/homebrew-tap) formula installs the latest release with its man pages and shell completions:

  ```sh
  brew install pimalaya/tap/carillon
  ```
</details>

<details>
  <summary>Pre-built binary</summary>

  For other systems, Carillon can be installed with the installer:

  *As root:*

  ```sh
  curl -sSL https://raw.githubusercontent.com/pimalaya/carillon/master/install.sh | sudo sh
  ```

  *As a regular user:*

  ```sh
  curl -sSL https://raw.githubusercontent.com/pimalaya/carillon/master/install.sh | PREFIX=~/.local sh
  ```

  These commands install the latest [release](https://github.com/pimalaya/carillon/releases), built with the default cargo features. For a build of `master`, check out the [releases](https://github.com/pimalaya/carillon/actions/workflows/releases.yml) GitHub workflow and look for the *Artifacts* section.
</details>

<details>
  <summary>Cargo</summary>

  ```sh
  cargo install --locked carillon
  ```

  For a build of `master`:

  ```sh
  cargo install --locked --git https://github.com/pimalaya/carillon.git
  ```
</details>

<details>
  <summary>Nix</summary>

  ```sh
  nix profile install github:pimalaya/carillon
  ```
</details>

<details>
  <summary>Sources</summary>

  Requires [Rust](https://www.rust-lang.org/tools/install) 1.89 or later:

  ```sh
  git clone https://github.com/pimalaya/carillon
  cd carillon
  cargo build --release --locked
  ```

  The binary lands at target/release/carillon.
</details>

## Configuration

A TOML configuration is loaded from every existing path among the following, each one deep-merged on top of the ones before:

- `$XDG_CONFIG_HOME/carillon/config.toml`
- `$HOME/.config/carillon/config.toml`
- `$HOME/.carillonrc`

Override the path with `-c <PATH>`, repeated to merge several files.

> [!TIP]
> See [config.sample.toml](./config.sample.toml) for a complete configuration of one example account with commented fields; rename the account and uncomment what you need, or start from a provider below.

An account declares the backend it watches, the collection it watches there and the hooks it fires, each named `on-<subject>-<verb>`. Carillon never issues nor refreshes an OAuth 2.0 token, so Google and Microsoft accounts read theirs from [Ortie](https://github.com/pimalaya/ortie).

### Google

Watch the inbox through the Gmail API, with a token from Ortie's Google account, or a calendar through `gcal` with the same token.

```toml
[accounts.google]

gmail.mailbox = "INBOX"
gmail.auth.token.command = ["ortie", "token", "show", "-a", "google"]
gmail.hooks.on-message-add.notify.summary = "New mail from $sender"
gmail.hooks.on-message-add.notify.body = "$subject"
```

### Microsoft

Watch the inbox and the calendar through Microsoft Graph, with a token from Ortie's Graph account.

```toml
[accounts.microsoft]

msgraph.mailbox = "inbox"
msgraph.calendar = "Calendar"
msgraph.auth.token.command = ["ortie", "token", "show", "-a", "microsoft-graph"]
msgraph.hooks.on-message-add.notify.summary = "New mail from $sender"
msgraph.hooks.on-event-change.notify.summary = "Event changed in $calendar"
```

### Fastmail

Watch the inbox over the JMAP event stream, with a token from Ortie or an [API token](https://www.fastmail.help/hc/en-us/articles/5254602856719).

```toml
[accounts.fastmail]

jmap.mailbox = "INBOX"
jmap.server = "https://api.fastmail.com/jmap/session"
jmap.auth.bearer.token.command = ["ortie", "token", "show", "-a", "fastmail"]
jmap.hooks.on-message-add.notify.summary = "New mail from $sender"
jmap.hooks.on-message-add.notify.body = "$subject"
```

## Usage

**Step 1**: once your first account is configured, check that every backend connects and authenticates:

```sh
carillon account check
```

**Step 2**: watch every account, firing its hooks, until interrupted:

```sh
carillon watch
```

A watch covers every account and any other command the default one, unless `-a` names another, and the `--help` of each command details its flags, exit codes and JSON output. More use cases:

```sh
# Watch the work account only
carillon -a work watch

# Pick the backend of an account declaring several
carillon -b jmap watch

# Feed a status bar with every change
carillon watch --json | jq -r --unbuffered 'select(.event == "changed") | "\(.account): \(.change.kind)"'

# List the accounts and the backends each declares
carillon account list
```

## Alternatives

- [goimapnotify](https://gitlab.com/shackra/goimapnotify): runs a command on IMAP IDLE events, IMAP only
- [inotifywait](https://github.com/inotify-tools/inotify-tools): watches a local Maildir for new files, Linux only

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
