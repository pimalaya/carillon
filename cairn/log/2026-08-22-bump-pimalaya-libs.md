---
cairn: log
change: bump-pimalaya-libs
landed: 2026-08-22
---

# Bring the Pimalaya dependencies up to date

io-imap 0.2 to 0.5, pimalaya-stream 0.1 to 0.3, pimalaya-cli 0.1 to 0.2, pimalaya-config to 0.1.4, and the MSRV to 1.89, which pimalaya-config 0.1.4 requires and which himalaya already declares.

## What landed

io-imap and pimalaya-cli needed no source change: the daemon composes the coroutines those releases kept, and it uses none of the renamed command types.

pimalaya-stream did. `StreamStd` is `stream::Stream`, `connect_tls` takes a `TlsConnectOptions` rather than a bare `Tls`, and reads now retry a not-ready socket for a minute by default. That default is right for a client running an exchange and wrong here: the driver arms a one-second read deadline precisely to be woken up, so it can check its shutdown flag between IDLE keep-alives, and a strategy that retried those wakeups away would leave a Ctrl+C unread until the server next spoke. `net::connect` opens with `Retry::Never`, which is the case the stream release names for exactly this shape of caller.

## Verification

Build, test, clippy and fmt green under the nix devshell.

## Spec

The transport requirement now states the retry stance and why, with a scenario for a Ctrl+C during a quiet IDLE.

## Not done

io-imap 0.5's `session::ImapSessionOpen` covers greeting, authentication, STARTTLS ordering and the provider quirks (Coremail's SASL-IR lie, PREAUTH detection) that this daemon hand-rolls in its own phases. Adopting it means restructuring the driver around transport-shaped yields, which is a change of its own. Same for pimalaya-config's `notify` serde adapter: it reads a ready `notify_rust::Notification`, whereas this config holds summary and body as templates expanded per message, so the adapter would not fit without turning templates into a notification first.
