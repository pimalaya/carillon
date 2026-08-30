---
cairn: log
change: alpn-under-each-backend
landed: 2026-08-29
---

# Gave every TLS backend its own ALPN key

## What landed

`From<TlsConfig> for Tls` is gone, replaced by `TlsConfig::into_tls(self, alpn)`, the signature himalaya, himalaya-tui, cardamum and neverest already carry. The old impl hardcoded an empty ALPN list, and each of the three call sites patched its own back in afterwards; nothing forced a fourth to, so the next connection added would have negotiated no ALPN silently.

`imap`, `jmap`, `caldav` and `carddav` each gained an `alpn: Option<Vec<String>>` key. Unset takes the default its client crate owns, `io_imap::session::default_alpn` over IMAP and the two `default_alpn` associated functions of io-jmap and io-http over the rest, so the three literals the source used to carry are gone and the defaults have one owner. An empty list skips ALPN negotiation and a non-empty one replaces the default; the key is omitted from a generated document when it is unset. Maildir gained none: it opens no socket.

`tls.cert` is now expanded when the file is read, through a private `opt_shell_expanded_path` carrying a TODO for the shared helper pimalaya-config has not shipped yet. It was the one path in the configuration read raw, so `cert = "~/certs/example.pem"` named a relative `./~/certs` directory that does not exist and the certificate was never found.

The DAV wizard's connection test builds its TLS handle the same way a watch does, rather than from its own `Tls::default()` plus a literal, so testing an account and watching it negotiate the same thing.

## What is still true

A configuration that names no `alpn` connects exactly as it did: the defaults are the literals that were in the source. The user's four accounts load unchanged, and so does the same file with `alpn = []`, `alpn = ["http/1.1", "h2"]` and a tilde certificate path added.

The discovery client's own TLS profile in the wizard's search module is untouched: it belongs to io-pim-discovery's well-known probes, not to a configured backend, and has no configuration to read.

## Note for verification

`cargo check --all-features` cannot build in this tree: the committed `[patch.crates-io]` entry points io-webdav at git, whose `WebdavSyncCollection::new` has grown an options argument `dav.rs` does not pass. That breakage predates this change and belongs to the io-webdav migration. Against the released io-webdav 0.2.1, every feature builds, clippy is clean and the whole suite passes.
