# Migration guide

## From draft to v0.1.0

### draft issues

- **Async runtime overkill for a single watch loop.** Mirador draft pulled in `tokio` for what is in practice one long-lived watch loop per process. The runtime cost (binary size, build time, dependency surface) was high for very little benefit.
- **Single backend abstraction was too rigid.** draft reused the legacy `email-lib` watch trait, which assumed every backend behaves like IMAP IDLE + a refetch. JMAP push (`Email/state` deltas) and Maildir filesystem events did not fit naturally.
- **Configuration drifted from the rest of the ecosystem.** The `backend.host` + `backend.port` + `backend.encryption` triple did not match the URL-based shape `himalaya` CLI v2 and `himalaya-tui` settled on.
- **Hook surface was thin.** Only `on-message-added` was wired; expunges and flag toggles were silent.
- **Native keyring and OAuth were in-binary.** Platform-specific bugs, silent failures, locked sessions.

### v0.1.0 changes

- **Deep rewrite on top of the I/O-free io-* ecosystem.** The CLI is synchronous, the binary is smaller, and each backend watches through the crate that speaks its own protocol.
- **Four first-class backends behind one watch surface:** IMAP holds an idle connection (RFC 2177), JMAP is pushed to over an event-source stream (RFC 8620 §7.3), Maildir re-lists the mailbox, and WebDAV asks a collection what moved since a sync token (RFC 6578), which covers CalDAV and CardDAV.
- **Shared configuration schema** with `himalaya` CLI v2 and `himalaya-tui` for the `[accounts.<name>]` block (`imap.*`, `jmap.*`, `maildir.*`, `dav.*` keys). One TOML file can back all three binaries.
- **Five watch event hooks** (`on-item-added`, `on-item-removed`, `on-item-changed`, `on-flags-added`, `on-flags-removed`) with template placeholders shared across kinds. `on-message-added` and `on-message-removed` are accepted as the former names of the first two.
- **Keyring moved out** to a shell command: any CLI that prints the secret to stdout works (`secret-tool lookup …`, `pass show …`, `security find-generic-password …`, etc.).
- **OAuth moved out** to [pimalaya/ortie](https://github.com/pimalaya/ortie) (or any external broker exposed via a shell command). SASL `oauthbearer` / `xoauth2` reads the token from the configured `command`.
- **TLS selectable at build time** between `rustls-ring` (default), `rustls-aws` and `native-tls`.

### CLI changes

#### Global flags

| draft | v0.1.0 |
|---|---|
| `--debug` (alias for `RUST_LOG=debug`) | `--log-level debug` (alias `--log`) |
| `--trace` (alias for `RUST_LOG=trace` + backtrace) | `--log-level trace` |
| (none) | `--json` (replaces draft plain-text-only output) |
| (none) | `-b/--backend {auto,imap,jmap,maildir,dav}` (force a specific backend when the account declares more than one) |

New in v0.1.0: `--log-file <PATH>` writes logs straight to a file, inherited from `pimalaya_cli::log::Logger`.

#### Subcommands

`-a/--account NAME` is now a global flag (placed before the subcommand: `mirador -a work watch`).

| draft | v0.1.0 |
|---|---|
| `watch [ACCOUNT] [FOLDER]` (positional) | `watch` (account is the global `-a` flag; what is watched is the account's `collection`, so there is no flag for it) |
| `doctor [ACCOUNT]` (aliases `check-up`, `checkup`, `check`) | `check` (validates the account against each configured backend; no aliases) |
| `configure [ACCOUNT] [--reset/-r]` | (removed; hand-edit [config.sample.toml](./config.sample.toml)) |
| `manual <SHELL>` (aliases `manuals`, `mans`) | `manuals <DIR>` (writes one man page per command) |
| `completion <SHELL>` (alias `completions`) | `completions <DIR>` (writes one completion script per shell) |

### Configuration changes

The full v0.1.0 schema lives in [config.sample.toml](./config.sample.toml). The notes below focus on the deltas.

#### Backend block

The single biggest shape change. The draft `[accounts.<name>.backend]` table is gone. v0.1.0 borrows the shape of [himalaya CLI v2](https://github.com/pimalaya/himalaya/blob/master/config.sample.toml): each backend lives under its own protocol key (`imap.*`, `jmap.*`, `maildir.*`, `dav.*`), declared as flat dotted entries directly under `[accounts.<name>]`. Declaring more than one is allowed; pick the active one with `-b/--backend {imap,jmap,maildir,dav}` (default `auto` picks IMAP, then JMAP, then Maildir, then WebDAV among the configured blocks). The mirador-only keys are `collection` (what the account watches, required) and `watch` (how it watches).

| draft | v0.1.0 |
|---|---|
| `backend.type = "imap"` + `backend.host` + `backend.port` + `backend.encryption = "tls" \| "start-tls" \| "none"` | `imap.server = "imaps://host[:port]"` (+ optional `imap.starttls = true`) |
| `backend.login = "..."` + `backend.auth.type = "password"` + `backend.auth.raw \| backend.auth.cmd \| backend.auth.keyring` | `imap.sasl.plain.authcid = "..."` + `imap.sasl.plain.passwd.raw = "..."` (or `.command = "pass show ..."`) |
| `backend.auth.type = "oauth2"` + `backend.auth.client-id` + `backend.auth.auth-url` + `backend.auth.token-url` | `imap.sasl.oauthbearer.username = "..."` + `imap.sasl.oauthbearer.host = "..."` + `imap.sasl.oauthbearer.port = 993` + `imap.sasl.oauthbearer.token.command = "ortie token read example"` (or `imap.sasl.xoauth2.*` for the Google variant) |
| `backend.type = "maildir"` + `backend.root-dir = "..."` | `maildir.root = "..."` |
| (no JMAP) | `jmap.server = "fastmail.com"` + `jmap.auth.bearer.token.command = "..."` (or `jmap.auth.basic.*` / `jmap.auth.header.*`) |

The IMAP SASL config carries the mechanism name as the *key* under `sasl` (`imap.sasl.plain.*`, `imap.sasl.oauthbearer.*`, …), not as a `type` field. Same for JMAP auth (`jmap.auth.bearer.*`, `jmap.auth.basic.*`, `jmap.auth.header.*`). Declare exactly one mechanism per backend.

#### Hooks

| draft | v0.1.0 |
|---|---|
| `on-message-added.notify.summary` / `.body` | `hooks.on-item-added.notify.summary` / `.body` |
| `on-message-added.cmd` | `hooks.on-item-added.cmd` |
| (no other event) | `hooks.on-item-removed.*`, `hooks.on-item-changed.*`, `hooks.on-flags-added.*`, `hooks.on-flags-removed.*` (same `notify` + `cmd` shape) |
| (no flag filter) | `hooks.on-flags-{added,removed}.flags = ["Seen", …]` narrows firing to a specific IANA-classified flag |

Placeholders use shell-style `$name` / `${name}` syntax in the notification `summary` / `body` (expanded with [subst](https://crates.io/crates/subst)) and are also exported as environment variables on the spawned `cmd` process. Available names: `id`, `collection` (`mailbox` under its former name), `subject`, `date`, `sender`, `sender_name`, `sender_address`, `recipient`, `recipient_name`, `recipient_address`, `flag`, `flags`. The envelope ones resolve on `hooks.on-item-added` only, and only over IMAP, since that is the backend that reads one.

The `cmd` field is decoded by [pimalaya-config](https://github.com/pimalaya/config) and accepts two TOML shapes: a **string** is handed to the platform shell (`/bin/sh -c <line>` on Unix, `cmd /C <line>` on Windows; quote placeholders as `"$subject"` so the shell expands them); a **list** `[program, args…]` is spawned directly with no shell (placeholders are still available as env vars to the spawned program).

#### Secrets

Every `*.passwd` / `*.token` field accepts either a raw literal (`{ raw = "…" }`) or a shell command (`{ command = "pass show foo" }` or `{ command = ["pass", "show", "foo"] }`). The draft `keyring = "…"` shortcut is removed; point the `command` at any CLI that prints the secret to stdout (`secret-tool` on Linux, `security` on macOS, `cmdkey` on Windows, `pass`, `gopass`, or your own script).

### Suggested steps

1. Copy [config.sample.toml](./config.sample.toml) to ~/.config/mirador/config.toml (next to the draft file).
2. Port your account: rewrite the `backend.*` block per the table above, point your secret command at the same source your draft keyring entry was, and copy the draft `on-message-added` block over verbatim.
3. `mirador -a <account> check` to validate the connection (auth + handshake per configured backend).
4. `mirador -a <account> watch` once to confirm the watch starts cleanly.
5. Drop the draft config when you are happy with the v0.1.0 one.

### Looking for a feature that is gone?

- **Interactive `configure` wizard**: removed. Edit the configuration by hand using [config.sample.toml](./config.sample.toml) as a template. A Pimalaya-wide wizard rewrite will eventually be plugged in elsewhere; mirador will consume it if/when it lands, but does not ship one itself.
- **In-binary keyring**: out. Point a shell `command = "…"` at any CLI that prints the secret to stdout (`secret-tool`, `security`, `cmdkey`, `pass`, `gopass`, or your own script).
- **In-binary OAuth 2 client**: out. Use [pimalaya/ortie](https://github.com/pimalaya/ortie) or any other broker, then point a `command = "..."` at the token.
- **Per-provider preset (Gmail, Outlook, iCloud, Proton Bridge)**: gone from the binary; the [Configuration](./README.md#configuration) section of the README will document a per-provider snippet table once the v0.1.0 series stabilises.
