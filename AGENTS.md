# AGENTS.md: Cairn activation

This repository follows **Cairn**, a language- and tool-agnostic convention for keeping a living spec, reviewable change proposals, and an honest history next to the code. The full format and the step-by-step procedure live in the [pimalaya/cairn](https://github.com/pimalaya/cairn) repository (`CAIRN.md` and `GUIDE.md`). No tooling is required. You create and check the structure by reading and following the rules.

The Cairn artifacts live under `cairn/` at the repository root: `spec/` (current truth, one file per capability), `changes/` (in-flight proposals), and `log/` (dated history).

If you are an agent working in this repository, do the following **by default, without being asked**.

## 1. Before non-trivial work, propose

For anything beyond a trivial fix, create `cairn/changes/<change-id>/` (kebab-case id) with:

- `proposal.md`: *why* and *what* (frontmatter: `cairn: change`, `id`, `status: active`, `created`).
- `tasks.md`: the checklist.
- `delta.md`: what this changes in the spec (`ADDED`, `MODIFIED`, `REMOVED`).

Let the human review intent **before** you write code. Trivial fixes may skip this and go straight to landing.

## 2. After work lands, fold and log (never skip)

- Fold the change's delta into `cairn/spec/<capability>.md` so the spec always reflects current truth (append ADDED, replace MODIFIED, delete REMOVED).
- Append a dated entry `cairn/log/YYYY-MM-DD-<change-id>.md` describing what landed and which capabilities moved.
- Set the change `status: landed`.

> **The forcing rule:** a change that affects behaviour is not *done* until the spec is updated and the log entry is written.

## 3. Stay conformant

Check the structure yourself by reading: a `cairn/` root with `spec/`, `changes/` and `log/`; every Cairn file carrying a valid `cairn:` type (`spec`, `change`, `tasks`, `delta`, `log`); each change having `proposal.md` and `tasks.md`; kebab-case ids; literal delta headings (`## ADDED Requirements`, `## MODIFIED Requirements`, `## REMOVED Requirements`); and a `log/YYYY-MM-DD-<change-id>.md` entry for every landed change. Everything else (prose, naming, ordering, extra files) is free.
