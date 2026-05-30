# Migration guide

## From v1 to v2

### v1 issues

- **Async runtime overkill for a single watch loop.** Mirador v1 pulled in `tokio` for what is in practice one long-lived IDLE / fsnotify loop per process. The runtime cost (binary size, build time, dependency surface) was high for very little benefit.
- **Single backend abstraction was too rigid.** v1 reused the legacy `email-lib` watch trait, which assumed every backend behaves like IMAP IDLE + a refetch. JMAP push (`Email/state` deltas) and Maildir filesystem events did not fit naturally.
- **Configuration drifted from the rest of the ecosystem.** The `backend.host` + `backend.port` + `backend.encryption` triple did not match the URL-based shape `himalaya` CLI v2 and `himalaya-tui` settled on.
- **Hook surface was thin.** Only `on-message-added` was wired; expunges and flag toggles were silent.
- **Native keyring and OAuth were in-binary.** Platform-specific bugs, silent failures, locked sessions.

### v2 changes

- **Deep rewrite on top of the I/O-free `io-*` ecosystem.** The CLI is synchronous (`std::net`), the binary is smaller, and the same shared `EmailClientStd::watch_mailbox` API drives all three backends.
- **Three first-class backends behind one watch surface:** IMAP IDLE (RFC 2177), JMAP EventSource push (RFC 8620 §7.2) and Maildir filesystem notifications.
- **Shared configuration schema** with `himalaya` CLI v2 and `himalaya-tui` for the `[accounts.<name>]` block (`imap.*`, `jmap.*`, `maildir.*` keys). One TOML file can back all three binaries.
- **Four watch event hooks** (`on-message-added`, `on-message-removed`, `on-flags-added`, `on-flags-removed`) with template placeholders shared across kinds.
- **Keyring moved out** to a shell command (`{ command = "secret-tool lookup …" }`, [pimalaya/mimosa](https://github.com/pimalaya/mimosa), `pass`, `gopass`, …).
- **OAuth moved out** to [pimalaya/ortie](https://github.com/pimalaya/ortie) (or any external broker exposed via a shell command). SASL `oauthbearer` / `xoauth2` reads the token from the configured `command`.
- **TLS selectable at build time** between `rustls-ring` (default), `rustls-aws` and `native-tls`.

### CLI changes

#### Global flags

| v1 | v2 |
|---|---|
| `--debug` (alias for `RUST_LOG=debug`) | `--log-level debug` (alias `--log`) |
| `--trace` (alias for `RUST_LOG=trace` + backtrace) | `--log-level trace` |
| (none) | `--json` (replaces v1 plain-text-only output) |
| (none) | `-b/--backend {auto,imap,jmap,maildir}` (force a specific backend when the account declares more than one) |

New in v2: `--log-file <PATH>` writes logs straight to a file, inherited from `pimalaya_cli::log::Logger`.

#### Subcommands

`-a/--account NAME` is now a global flag (placed before the subcommand: `mirador -a work watch`).

| v1 | v2 |
|---|---|
| `watch [ACCOUNT] [FOLDER]` (positional) | `watch [-m/--mailbox NAME]` (account is the global `-a` flag; `folder` → `mailbox`) |
| `doctor [ACCOUNT]` (aliases `check-up`, `checkup`, `check`) | `check` (validates the account against each configured backend; no aliases) |
| `configure [ACCOUNT] [--reset/-r]` | (removed; hand-edit [config.sample.toml](./config.sample.toml)) |
| `manual <SHELL>` (aliases `manuals`, `mans`) | `manuals <DIR>` (writes one man page per command) |
| `completion <SHELL>` (alias `completions`) | `completions <DIR>` (writes one completion script per shell) |

### Configuration changes

The full v2 schema lives in [config.sample.toml](./config.sample.toml). The notes below focus on the deltas.

#### Backend block

The single biggest shape change. The v1 `[accounts.<name>.backend]` table is gone. v2 borrows the shape of [himalaya CLI v2](https://github.com/pimalaya/himalaya/blob/master/config.sample.toml): each backend lives under its own protocol key (`imap.*`, `jmap.*`, `maildir.*`), declared as flat dotted entries directly under `[accounts.<name>]`. Declaring more than one is allowed; pick the active one with `-b/--backend {imap,jmap,maildir}` (default `auto` picks IMAP, then JMAP, then Maildir among the configured blocks).

| v1 | v2 |
|---|---|
| `backend.type = "imap"` + `backend.host` + `backend.port` + `backend.encryption = "tls" \| "start-tls" \| "none"` | `imap.server = "imaps://host[:port]"` (+ optional `imap.starttls = true`) |
| `backend.login = "..."` + `backend.auth.type = "password"` + `backend.auth.raw \| backend.auth.cmd \| backend.auth.keyring` | `imap.sasl.plain.authcid = "..."` + `imap.sasl.plain.passwd.raw = "..."` (or `.command = "pass show ..."`) |
| `backend.auth.type = "oauth2"` + `backend.auth.client-id` + `backend.auth.auth-url` + `backend.auth.token-url` | `imap.sasl.oauthbearer.username = "..."` + `imap.sasl.oauthbearer.host = "..."` + `imap.sasl.oauthbearer.port = 993` + `imap.sasl.oauthbearer.token.command = "ortie token read example"` (or `imap.sasl.xoauth2.*` for the Google variant) |
| `backend.type = "maildir"` + `backend.root-dir = "..."` | `maildir.root = "..."` |
| (no JMAP) | `jmap.server = "fastmail.com"` + `jmap.auth.bearer.token.command = "..."` (or `jmap.auth.basic.*` / `jmap.auth.header.*`) |

The IMAP SASL config carries the mechanism name as the *key* under `sasl` (`imap.sasl.plain.*`, `imap.sasl.oauthbearer.*`, …), not as a `type` field. Same for JMAP auth (`jmap.auth.bearer.*`, `jmap.auth.basic.*`, `jmap.auth.header.*`). Declare exactly one mechanism per backend.

#### Hooks

| v1 | v2 |
|---|---|
| `on-message-added.notify.summary` / `.body` | unchanged |
| `on-message-added.cmd` | unchanged |
| (no other event) | `on-message-removed.*`, `on-flags-added.*`, `on-flags-removed.*` (same `notify` + `cmd` shape) |
| (no flag filter) | `on-flags-{added,removed}.flags = ["Seen", …]` narrows firing to a specific IANA-classified flag |

Placeholders use shell-style `$name` / `${name}` syntax and are available everywhere: `id`, `mailbox`, `subject`, `sender`, `sender_name`, `sender_address`, `recipient`, `recipient_name`, `recipient_address`, `flag`, `flags`. Sender / recipient / subject only resolve on `on-message-added` (the other event kinds carry just the id). Shell-command hooks receive them as environment variables, so the shell's own expansion does the substitution: quote references as `"$subject"` for safe whitespace handling.

#### Secrets

Every `*.passwd` / `*.token` field accepts either a raw literal (`{ raw = "…" }`) or a shell command (`{ command = "pass show foo" }` or `{ command = ["pass", "show", "foo"] }`). The v1 `keyring = "…"` shortcut is removed; use `secret-tool` (Linux), `security` (macOS), `cmdkey` (Windows), `pass`, `gopass` or [pimalaya/mimosa](https://github.com/pimalaya/mimosa) as the source command.

### Suggested steps

1. Copy [config.sample.toml](./config.sample.toml) to `~/.config/mirador/config.toml` (next to the v1 file).
2. Port your account: rewrite the `backend.*` block per the table above, point your secret command at the same source your v1 keyring entry was, and copy the v1 `on-message-added` block over verbatim.
3. `mirador -a <account> check` to validate the connection (auth + handshake per configured backend).
4. `mirador -a <account> watch` once to confirm the IDLE / SSE / fsnotify loop starts cleanly.
5. Drop the v1 config when you are happy with the v2 one.

### Looking for a feature that is gone?

- **Interactive `configure` wizard**: removed. Edit `config.toml` by hand using [config.sample.toml](./config.sample.toml) as a template. A Pimalaya-wide wizard rewrite will eventually be plugged in elsewhere; mirador will consume it if/when it lands, but does not ship one itself.
- **In-binary keyring**: out. Use a shell command (`secret-tool`, `pass`, `mimosa`, …).
- **In-binary OAuth 2 client**: out. Use [pimalaya/ortie](https://github.com/pimalaya/ortie) or any other broker, then point a `command = "..."` at the token.
- **Per-provider preset (Gmail, Outlook, iCloud, Proton Bridge)**: gone from the binary; the [Configuration](./README.md#configuration) section of the README will document a per-provider snippet table once the v2 series stabilises.
