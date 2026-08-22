# Contributing guide

Thank you for investing your time in contributing to Mirador.

Whether you are a human or an AI agent, read these in order before touching the code:

1. the [Pimalaya README](https://github.com/pimalaya) for what the project is and how its repositories stack;
2. the [Pimalaya CONTRIBUTING](https://github.com/pimalaya/.github/blob/master/CONTRIBUTING.md) guide, which chains to the shared architecture and guidelines;
3. the inline header documentation, starting with src/main.rs: it is the architecture document of this crate;
4. the cairn/ folder, which follows [Cairn](https://github.com/pimalaya/cairn): `spec/` is current truth, `changes/` holds in-flight proposals, and `log/` is the dated history.

Everything below documents only what differs from the Pimalaya standards.

## Where changes belong

Mirador owns no protocol code. It supervises watches, reads the configuration and fires the hooks, and everything on the wire belongs to the crate that speaks that protocol. Triage before patching:

- how a change is learned belongs to the protocol crate: [io-imap](https://github.com/pimalaya/io-imap) for the idle watch, [io-jmap](https://github.com/pimalaya/io-jmap) for the changes poll, [io-maildir](https://github.com/pimalaya/io-maildir) for the listing poll, [io-webdav](https://github.com/pimalaya/io-webdav) for the collection poll;
- the shared TOML shape, since one file backs three binaries, is settled with [himalaya](https://github.com/pimalaya/himalaya) rather than here;
- the configuration schema, the backend selection, the supervision and the hooks live here.

The clap, printer and logger primitives come from [pimalaya/cli](https://github.com/pimalaya/cli), the TOML loader and the secret resolution from [pimalaya/config](https://github.com/pimalaya/config), and the TCP and TLS plumbing from [pimalaya/stream](https://github.com/pimalaya/stream).

## Feature matrix

Every backend is a cargo feature, and so is every TLS provider. A change touching one of them is built against the others before it lands, since the default set hides what a narrower one breaks:

```sh
cargo build --no-default-features --features imap,rustls-ring
cargo build --no-default-features --features jmap,native-tls
cargo build --no-default-features --features maildir
cargo build --no-default-features --features dav,rustls-aws
```

## Watching against a real server

There is no test suite for a watch: it needs a server, a collection and something moving in it. A change to a backend is verified by hand against a real account, `mirador -a <account> check` first for the connection, then `mirador -a <account> watch` with the event provoked from another client. Report what you ran against in the change proposal, since providers disagree on what they advertise and on what they then honour.
