---
cairn: log
change: guidelines-alignment
landed: 2026-08-22
---

# Align the repository with the Pimalaya guidelines

A pass over the whole repository against .github/GUIDELINES.md, rule by rule.

## What landed

The driver module is now supervisor, which is what its own header called it all along. The word is banned across Pimalaya, and it had also reached two lines of wizard documentation ("email-driven discovery", "it drives the prompt") and one changelog entry; all of them now say what they mean instead. The cairn log entries keep their wording, being immutable history.

Three code rules were failing. The configuration module carried eight dashed section banners, which inline-005 bans outright: the types they separated already have doc comments, which is what navigation is supposed to rest on. Twenty-seven bare `//` comments carried no tag, and every one of them turned out to be a NOTE. Twenty-five user-facing error messages started lowercase, against naming-012, while the log messages that must stay lowercase already did.

One rule was failing in a way a user would feel. cli-002 asks that a failure to resolve an account name what is missing and what to do about it, and carillon answered "Account `x` not found in config" without saying which accounts exist, or "Cannot find account" without saying that `-a` and `default = true` are the two ways to pick one. Both now say it, which is the only behaviour this pass changed.

The manifest lost the docs.rs metadata block a binary publishes nothing to, sorted its dependencies, and took the org author address. The README features section was rewritten as user features rather than an implementation tour, and shortened; the coverage table is sorted by spec name and gained the four discovery mechanisms the wizard actually uses. The configuration section follows the documented path shape, and paths lost their backticks there and in CONTRIBUTING.md.

The changelog was reshaped to changelog-001, every entry now a one-line summary with its detail in indented paragraphs. Reading it that way surfaced two entries that had become false: one announced the removal of the wizard and its discovery dependency, both of which came back the same day, and one said Cargo.toml patches io-imap to a local path, which it no longer does. changelog-002 is why they had to go: a section reports the net state, and history belongs here.

Every feature combination now compiles warning-free. A build enabling no backend at all still compiles the change vocabulary and the hook runner nothing then feeds, so the build script sets a `backend` cfg and main.rs allows the dead code in that one configuration, rather than gating every item of two modules on four features.

## Verification

Build, clippy and fmt green on the empty, imap, jmap, maildir, dav and full feature sets; 41 tests pass. The sample configuration was loaded by `carillon check` to prove it still parses after the edits.

## Not done

AGENTS.md keeps its backticked file names. It is vendored from the Cairn convention repository, and the cost of diverging from upstream is higher than the rule it breaks.

## Follow-up: the module went away rather than being renamed

The rename landed as supervisor, and supervisor is the same kind of name as driver: it says what the code plays rather than what it is about, which no other module here does (config, event, hook, backend, watch are all subjects).

So the code moved instead. `watch_account` and `watch_session` now live in watch.rs, next to the command whose threads run them, which was their one and only caller. Nothing is named after a role any more, and there is one module fewer: the file reads as the command, the accounts it selects, what each thread does, and the helpers under it.
