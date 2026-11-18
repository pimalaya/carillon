---
cairn: log
change: imap-envelope-encoded-words
landed: 2026-11-18
---

# IMAP envelope text decoded

Finding CRL-4 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4.5), shown on 2026-11-16 by a run against a loopback IMAP server. Spec moved: daemon (Arrivals are resolved only when a hook wants them, a sentence and a scenario).

- `imap::summarize` read the ENVELOPE subject and the from and to names through `nstring`, raw, so a hook received `=?UTF-8?Q?R=C3=A9union?=` where JMAP, Graph and himalaya hand `Réunion`; `$sender` and `$recipient`, composed from the name, carried the encoded word too. The subject and the names now go through `text`, which reads them with mime-rs's `codec::rfc2047::decode_text`: adjacent words of one charset joined before transcoding, the blanks between them dropped, a word that does not parse kept as written. The date and the address parts stay raw.
- mime-rs 0.2 was already in the tree through io-imap, with the same `base64` and `encoding` features; Cargo.toml now names it under the `imap` feature (`dep:mime-rs`), and Cargo.lock adds it to carillon's dependency list alone. himalaya decodes with its own `decode_words` over mime-rs's codecs; mime-rs 0.2 now ships the same reading, so carillon takes it rather than a copy.
- Test: src/imap.rs `an_envelope_is_read_with_its_encoded_words_decoded` (a Q subject in two words, an ISO-8859-1 sender name, a B recipient name) fails on the HEAD code (6c28a39, the subject read raw) and passes after.
