---
cairn: log
change: the-installer-fetches-the-release
landed: 2026-11-18
---

# The installer fetches the release

Line PKG-2 of the family analysis (~/.claude/plans/pimalaya-cli-family-analysis-2026-11-15/plan.md section 4), carillon's part. No spec moved.

- On 77b31c6 install.sh fetched from github.com/soywod/carillon/releases and asked for `carillon.arm64-linux.tgz`, `carillon.x86_64-macos.tgz` and `carillon.arm64-macos.tgz`, where pimalaya/nix's releases.yml, which carillon's workflow calls, publishes `<project>.<target>.tgz` for aarch64-darwin, aarch64-linux, armv6l-linux, armv7l-linux, i686-linux, x86_64-darwin, x86_64-linux and x86_64-windows: the install failed on every Mac and on arm64 Linux. The script now fetches from pimalaya/carillon and asks the published names, with the i686, armv6l and armv7l Linux cases neverest's install.sh has; the two scripts now differ by the project name and by the sample copy neverest's adds (PKG-6, next).
- Left as neverest has it: `curl -sLo` without `-f`, so a 404 page is saved and `tar` then fails, and no checksum. Both are the same in the family's installers; changing them is one move for all of them, not carillon's alone.
- Checked: `sh -n install.sh`, and the asset names against pimalaya/nix .github/workflows/releases.yml and release.yml. Not run, so nothing was downloaded.
