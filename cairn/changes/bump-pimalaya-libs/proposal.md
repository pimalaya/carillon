---
cairn: change
id: bump-pimalaya-libs
status: landed
created: 2026-08-22
---

# Bring the Pimalaya dependencies up to date

## Why

The daemon was still on io-imap 0.2, pimalaya-stream 0.1, pimalaya-cli 0.1 and pimalaya-config 0.1.1 while the org had moved to 0.5, 0.3, 0.2 and 0.1.4. Sitting behind on the org's own libraries is how a consumer ends up with a private fork of a fix.

One of those bumps is not mechanical. pimalaya-stream 0.3 made reads retry a not-ready socket for a minute by default, which is right for a client running an exchange and wrong for this daemon: it arms a one-second read deadline precisely to be woken up, so it can check its shutdown flag between IDLE keep-alives. Under the default strategy a Ctrl+C would wait for the server to speak.

## What

- io-imap 0.2 to 0.5, pimalaya-stream 0.1 to 0.3, pimalaya-cli 0.1 to 0.2, pimalaya-config to 0.1.4, and the MSRV to 1.89 which pimalaya-config 0.1.4 requires.
- `net::connect` opens with `Retry::Never`, taking the not-ready failures back, and ports to stream 0.3's renamed types and options structs.

## Not done

io-imap 0.5 ships `session::ImapSessionOpen`, a composite coroutine covering greeting, authentication, STARTTLS ordering and the provider quirks this daemon currently hand-rolls in its own greet-then-authenticate phases. Adopting it would restructure the driver around transport-shaped yields, which is a change of its own rather than a bump.
