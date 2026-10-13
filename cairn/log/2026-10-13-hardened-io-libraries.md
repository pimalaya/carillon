---
cairn: log
change: hardened-io-libraries
landed: 2026-10-13
---

# Protocol libraries pinned to their hardening fixes

`[patch.crates-io]` pins three libraries to unreleased fixes, until releases carry them:

- io-http e181530: response heads capped at 64 KiB and bodies at 256 MiB, `Eof` on a connection closed mid-body instead of an endless `WantsRead`, no panic on a huge chunk size.
- io-imap b1ab38e: no panic on a greeting carrying a literal, FETCH and STARTTLS lines scanned incrementally under a cap, LOGIN and AUTHENTICATE arguments kept out of trace logs.
- io-webdav 62fad89: a multistatus parsed in linear time, nesting capped at 64.

io-gmail 0.5, io-gpeople 0.5, io-gcal 0.2 and io-msgraph 0.6 (pinned to 2ae2637) follow, the versions Neverest builds with. The watch reads API errors through `status()`, which every one of them keeps, so no code changed: throttling, backoff, the expired-cursor statuses (Gmail 404, Graph and People 410) and the hooks behave as before.

Capabilities moved: none.

Verified: `cargo build`, `clippy --all-targets` and `test`, with the defaults and with MOA's features (`rustls-ring imap jmap dav msgraph gmail gcal gpeople`). MOA's `pimdir_demo` (IMAP IDLE) and `watch_demo` (CalDAV) pass against this build on a local Stalwart.
