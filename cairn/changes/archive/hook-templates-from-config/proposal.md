---
cairn: change
id: hook-templates-from-config
status: landed
created: 2026-11-05
---

# Hook templates from pimalaya-config

## Why

carillon expanded its notifications with subst (`${name:default}`) while the `command` of the same hook got the same variables in a shell, where the default is `${name:-default}`: two syntaxes in one hook. Pimalaya guideline cli-008 gives every hook-based product one shape, camelCase variables in the shell's syntax, and pimalaya-config now carries it (`notify::NotificationTemplate`, pimalaya-config log 2026-11-05-notification-template). Split out of hook-shape-follows-siblings, which keeps the renames of hooks and keys.

## What

- The `notify` table of every hook is a pimalaya-config `NotificationTemplate`, checked against the hook's vocabulary when the configuration is read and rendered when the hook fires. carillon's `NotifyConfig`, subst and its direct notify-rust dependency go.
- Variables are camelCase: `$senderName`, `$senderAddress`, `$recipientName`, `$recipientAddress`; the one-word ones keep their spelling. A command gets them in its environment under the same names.
- `${name:-default}` replaces `${name:default}`, which is refused when read. A default stands in for an empty value, never for a name the hook does not have: `${sendr:-x}` is refused rather than always showing `x`.
- A `notify` cargo feature, on by default, forwards to pimalaya-config's; a build without it refuses a `notify` table naming the feature, and its wizard writes hooks without one. `vendored` forwards to pimalaya-config's instead of notify-rust's.
- pimalaya-config moves to 0.3, through a path patch until the template is released.
