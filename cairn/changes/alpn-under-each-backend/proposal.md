---
cairn: change
id: alpn-under-each-backend
status: landed
created: 2026-08-29
---

# Give every TLS backend its own ALPN key

## Why

Three call sites built a `Tls` out of a `TlsConfig` and then patched the ALPN list back into it, because `From<TlsConfig> for Tls` hardcoded an empty one. Nothing forced a fourth call site to do the same, so the next connection added would have negotiated no ALPN and nobody would have noticed until a server that needs one refused it.

The three lists were also literals in the source, which is the wrong place: `vec!["imap"]` and `vec!["http/1.1"]` are exactly what a configuration key should be able to override. A server whose TLS terminator rejects a handshake carrying `http/1.1` has no way to say so, and the rest of the family (himalaya, himalaya-tui, cardamum, neverest) already exposes that key.

While reading the same table, `tls.cert` turned out to be the one path in the configuration that is not expanded when the file is read, so `cert = "~/certs/example.pem"` names a relative `./~/certs` directory that does not exist.

## What

- `From<TlsConfig> for Tls` becomes `TlsConfig::into_tls(self, alpn)`, the signature himalaya, himalaya-tui, cardamum and neverest already carry. A call site now has to say what it negotiates.
- `imap`, `jmap`, `caldav` and `carddav` each gain an `alpn: Option<Vec<String>>` key. Unset takes the default its client crate owns, `[]` skips ALPN, and a non-empty list replaces the default. Maildir gains none: it opens no socket.
- `tls.cert` is expanded at deserialize, through a private `opt_shell_expanded_path` carrying a TODO for the shared helper pimalaya-config has not shipped yet.
- The DAV wizard's test connection opens with the same profile a watch does, rather than with its own literal.
