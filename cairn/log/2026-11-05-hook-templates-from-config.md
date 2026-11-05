---
cairn: log
change: hook-templates-from-config
landed: 2026-11-05
---

# Hook templates from pimalaya-config

Hook notifications are pimalaya-config `NotificationTemplate`s (pimalaya-config 0.3, path patch until released): src/config.rs drops `NotifyConfig`, src/hook.rs checks a template against the hook's `Vocabulary` at load and renders it at firing, subst and the direct notify-rust dependency are gone. Envelope variables are camelCase in notifications and command environments. A `notify` cargo feature (default) forwards to pimalaya-config's, `vendored` forwards there too, and the wizard writes hooks without a notification in a build without it (`ItemHook::notifying`). Spec moved: `daemon` (MODIFIED: A hook templates against what its event carries; ADDED: Notifications are shown when the build can show them).

A default used to let any name through (`${sendr:default}` always showed its default); it now stands in for an empty value only, so a misspelt name is refused. The error a refused template gives names the hook's `notify` table rather than its summary or body, pimalaya-config's check covering both.
