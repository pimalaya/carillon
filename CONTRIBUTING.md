# Contributing guide

Thank you for investing your time in contributing to Carillon.

Whether you are a human or an AI agent, read these in order before touching the code:

1. the [Pimalaya README](https://github.com/pimalaya) for what the project is and how its repositories stack;
2. the [Pimalaya CONTRIBUTING](https://github.com/pimalaya/.github/blob/master/CONTRIBUTING.md) guide, which chains to the shared architecture and guidelines;
3. the inline header documentation, starting with src/main.rs: it is the architecture document of this crate;
4. the cairn/ folder, which follows [Cairn](https://github.com/pimalaya/cairn): spec/ is current truth, changes/ holds in-flight proposals (landed ones move to changes/archive/), and log/ is the dated history; `sh cairn/verify.sh` checks the structure.

Everything below documents only what differs from the Pimalaya standards.

## Where changes belong

Carillon owns no protocol code. It supervises watches, reads the configuration and fires the hooks, and everything on the wire belongs to the crate that speaks that protocol. Triage before patching:

- how a change is learned belongs to the protocol crate: [io-imap](https://github.com/pimalaya/io-imap) for the idle watch, [io-jmap](https://github.com/pimalaya/io-jmap) for the changes poll, [io-maildir](https://github.com/pimalaya/io-maildir) for the listing poll, [io-webdav](https://github.com/pimalaya/io-webdav) for the collection poll;
- the configuration schema, the backend selection, the supervision and the hooks live here.

The clap, printer and logger primitives come from [pimalaya/cli](https://github.com/pimalaya/cli), the TOML loader and the secret resolution from [pimalaya/config](https://github.com/pimalaya/config), and the TCP and TLS plumbing from [pimalaya/stream](https://github.com/pimalaya/stream).

## Feature matrix

Every backend is a cargo feature named as its configuration block (`caldav` and `carddav` build apart, `caldav` alone reading the alarms of a calendar), and so is every TLS provider. A change touching one of them is built against the others before it lands, since the default set hides what a narrower one breaks:

```sh
cargo build --no-default-features --features imap,rustls-ring
cargo build --no-default-features --features jmap,native-tls
cargo build --no-default-features --features maildir
cargo build --no-default-features --features caldav,rustls-aws
```

The CI `features` job of .github/workflows/tests.yml runs clippy over these sets, the default and all features, and each backend alone with rustls-ring, then the tests of a build without imap (tests/backend_not_built.rs), which leaves both DAV backends out too.

## Watching against a real server

`cargo test` runs offline, a local Maildir standing in for a server. A watch over a network backend needs a server, a collection and something moving in it, so the suites proving one are live and ignored by default: tests/google.rs, tests/msgraph.rs and tests/jmap.rs.

The JMAP suite runs against a local Stalwart provisioned by tests/stalwart.sh, and Fastmail. Each suite's header names the credentials it reads and the command running it, one thread at a time.

A backend without a live suite is verified by hand against a real account: `carillon account check -a <account>`, then `carillon watch -a <account>` with the event provoked from another client. Report what you ran against in the change proposal.

In CI, the offline jobs run on every push. The live job never runs on push: it runs weekly (Mondays, 06:00 UTC) and by hand with `gh workflow run tests.yml`, and a release runs it on the commit to tag. The Google calendar test works in a long-lived calendar, `pimalaya-live-carillon-hooks`, emptied at the start and end of a run and never deleted.
