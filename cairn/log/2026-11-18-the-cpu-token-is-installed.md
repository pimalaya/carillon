---
cairn: log
change: the-cpu-token-is-installed
landed: 2026-11-18
---

# The CPU token is installed

Line COH-12 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4), carillon's half; neverest's landed on 2026-11-17 (neverest/cairn/log/2026-11-17-the-cpu-token-is-installed.md), the precedent this follows. No spec moved: the token chooses a codec back end, not what a watch reports.

- On 056ad44 carillon/src/main.rs installed no CPU token, so the codecs it reaches through mime-rs (IMAP envelope words and Gmail headers, RFC 2047 B and Q) and through io-pimdir ran their portable baseline. `main` now calls `SimdCpu::install_detected()` first, as himalaya, cardamum and neverest do, and Cargo.toml names `simd-cpu = { version = "0.1", default-features = false, features = ["std"] }`, the spelling neverest and himalaya use. simd-cpu 0.1.0 was already in Cargo.lock through base64-codec, mime-rs and quoted-printable-codec, so only carillon's dependency list moves.
- Not measured: no timing was taken, and no test was added, the token being a choice of back end the codecs' own tests cover.
- Also: tests/account.rs gates the `thread` and `time` imports its imap-only test added under COH-2, which a build without the `imap` feature warned about.
