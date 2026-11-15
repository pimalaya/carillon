---
cairn: change
id: guidelines-alignment
status: landed
created: 2026-08-22
---

# Align the repository with the Pimalaya guidelines

## Why

The org-wide guidelines (.github/GUIDELINES.md) are the settled conventions every repository is checked against, and carillon had drifted from several of them. The drift is not cosmetic in every case: a module named after a banned word teaches the wrong vocabulary to the next reader, a changelog entry describing a removal that has since been undone is simply false, and an account error that does not say what the configuration holds leaves the user guessing.

## What

- naming: the driver module goes away, its code folded into the watch command it served. The word is banned across the codebase, and the module had one caller, so nothing justified a module of its own; the two remaining uses in the wizard docs go with it.
- inline-005: the dashed section banners in the configuration module are removed. The types carry doc comments already, which is what the guideline says navigation should rest on.
- inline-004: every remaining bare `//` comment is tagged NOTE, the only tag any of them warranted.
- naming-012: user-facing error messages start with a capital, log messages stay lowercase.
- cli-002: a missing named account lists the accounts the configuration holds, and a missing default account names both ways of picking one, which `watch` and `check` did not do.
- cargo-008 and cargo-009: dependencies are alphabetical, and the docs.rs metadata block a binary has no use for is gone.
- changelog-001 and changelog-002: every entry opens with a one-line summary, with the detail in indented paragraphs, and the section reports the net state rather than its own history. Two entries described work that has since been undone.
- markdown-003 and readme-008 and readme-010: paths leave their backticks, the features read as user features rather than as an implementation tour, and the configuration paths follow the documented shape.
- every feature combination compiles warning-free, including the backendless one, through a `backend` cfg the build script sets.

## Not done

AGENTS.md keeps its backticked file names: it is vendored from the Cairn convention repository, and diverging from upstream to satisfy markdown-003 would cost more than it buys.
