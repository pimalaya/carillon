---
cairn: log
change: the-sample-is-shipped
landed: 2026-11-18
---

# The sample is shipped

Line PKG-6 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4), carillon's part; neverest's landed with 3ffdf17, the precedent this follows. No spec moved: daemon.md already has a first run point at config.sample.toml.

- On 68f0817 package.nix installed the service unit, completions, man pages and schemas but no config.sample.toml, and install.sh copied the binary alone, so pimalaya-cli's `shipped_sample`, which the welcome reads from share/doc/carillon/ beside the binary, found nothing in a Nix or a release install and offered only the download. package.nix's `postInstall` now runs `install -Dm644 config.sample.toml "$out"/share/doc/${finalAttrs.pname}/config.sample.toml`, neverest's line, and install.sh copies `share/doc/carillon/config.sample.toml` from the archive when it holds one, neverest's block. pimalaya/nix's release.yml packs `result/share` into each archive, so the release carries it from the next tag on.
- install.sh is now neverest's with the name changed.
- Checked: `nix-instantiate --parse package.nix`, `sh -n install.sh`. Not built: no `nix build` was run, and nothing was downloaded.
