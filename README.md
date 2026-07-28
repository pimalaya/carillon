# 🔔 carillon [![Matrix](https://img.shields.io/badge/chat-%23pimalaya-blue?style=flat&logo=matrix&logoColor=white)](https://matrix.to/#/#pimalaya:matrix.org) [![Mastodon](https://img.shields.io/badge/news-%40pimalaya-blue?style=flat&logo=mastodon&logoColor=white)](https://fosstodon.org/@pimalaya)

CLI daemon that watches PIM accounts and fires local consumers on each change

> [!WARNING]
> This project is pre-release and unpublished. Its config and behaviour may change without notice until the first tagged version.

carillon is the lightweight frontend of [Carillon](https://github.com/pimalaya): a self-hostable daemon that holds a standing IMAP IDLE connection and, the instant a mailbox changes, fires a local consumer. It is the same watch loop the Carillon server runs, without the network-and-trust apparatus. It signals; it never syncs, and it never sees message content it does not fetch on your behalf.

## Table of contents

- [Features](#features)
- [Installation](#installation)
- [Configuration](#configuration)
- [Usage](#usage)
- [AI disclosure](#ai-disclosure)
- [License](#license)
- [Social](#social)
- [Contributing](#contributing)
- [Sponsoring](#sponsoring)

## Features

- **Content-free watching**: holds IMAP IDLE and detects a change without reading message content, so nothing about your mail leaves your machine.
- **Desktop notifications**: pop a system notification the instant a watched mailbox changes.
- **Shell hooks**: run any command on a change, with the event details passed as environment variables, so a script can re-fetch or react however it likes.
- **Keyring-friendly credentials**: read the password or token from a command such as `pass` or `secret-tool`, never storing it in cleartext.
- **Multi-account config**: declare any number of accounts; pick which to watch with `-a <name>`.

## Installation

Pre-built binaries and package-manager recipes will land with the first release. For now, build from sources:

```sh
cargo install --git https://github.com/pimalaya/carillon
```

## Configuration

A configuration is loaded from the first valid path among:

- the path passed with `-c <PATH>` or `CARILLON_CONFIG=<PATH>`
- $XDG_CONFIG_HOME/carillon/config.toml
- $HOME/.config/carillon/config.toml
- ./carillon.toml

The full field reference lives in [config.sample.toml](./config.sample.toml).

## Usage

Watch the default account's mailbox until interrupted:

```sh
carillon watch
carillon watch -a work -m INBOX
```

Set `RUST_LOG=debug` for verbose logs. The complete command list is behind `carillon --help`.

## AI disclosure

This project is developed with AI assistance. This section documents how, so users and downstream packagers can make informed decisions.

- **Tools**: Claude Code (Anthropic), Opus 4.8, invoked locally with a persistent project-scoped memory and a small set of repo-specific rules.
- **Used for**: Refactors, mechanical multi-file edits, boilerplate (feature gates, error enums, derive macros, trait impls), test scaffolding, doc polish, exploratory design conversations.
- **Not used for**: Engineering, critical code, git manipulation (commit, merge, rebase…), real-world tests.
- **Verification**: Every AI-assisted change is read, compiled, tested, and formatted before commit (`nix develop --command cargo check / cargo test / cargo fmt`). Behavioural correctness is verified against the relevant RFC or upstream spec, not assumed from the model output. Tests are never adjusted to fit AI-generated code; the code is adjusted to fit correct behaviour.
- **Limitations**: AI models occasionally produce code that compiles and passes tests but is subtly wrong: off-by-one errors, missed edge cases, plausible but nonexistent APIs, stale RFC references. The verification workflow catches most of this; it does not catch all of it. Bug reports are welcome and taken seriously.
- **Last reviewed**: 28/07/2026

## License

This project is licensed under either of:

- [MIT license](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

## Social

- Chat on [Matrix](https://matrix.to/#/#pimalaya:matrix.org)
- News on [Mastodon](https://fosstodon.org/@pimalaya) or [RSS](https://fosstodon.org/@pimalaya.rss)
- Mail at [pimalaya.org@posteo.net](mailto:pimalaya.org@posteo.net)

## Contributing

Contributions are welcome: start with [CONTRIBUTING.md](https://github.com/pimalaya/.github/blob/master/CONTRIBUTING.md), which opens with the Pimalaya-wide guides to read first.

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
[![PayPal](https://img.shields.io/badge/-PayPal-0079c1?logo=PayPal&logoColor=ffffff)](https://www.paypal.com/paypalme/soywod)
