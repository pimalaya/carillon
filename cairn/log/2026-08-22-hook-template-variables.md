---
cairn: log
change: hook-template-variables
date: 2026-08-22
---

# A hook's notification is checked against what its event carries

A hook whose notification named a variable its event does not carry fired nothing and said nothing. `subst` refuses to expand a template with an unknown name, the runner logged the refusal at `warn` and moved on, and the command half of the same hook ran, so the hook half-fired and the half that went missing was the visible one. Found against a live Dovecot: an `on-message-removed` whose body read `$subject` reported the removal, resolved the hook, and left `notify hook failed: cannot expand body: No such variable: $subject` in a log nobody reads at that level.

Each hook now declares what it can fill, in one place both the loader and the runner read: `$id` and `$collection` everywhere, `$flag` on a flag hook, and the envelope names (`$subject`, `$date`, `$sender`, `$sender_name`, `$sender_address`, `$recipient`, `$recipient_name`, `$recipient_address`) on the IMAP arrival hook alone, that being the only one anything resolves an envelope for. Loading expands every `notify` summary and body against that vocabulary and nothing else, so the check is the expansion itself: it refuses exactly what a firing would, and `${name:default}` passes whatever the name, a default being how a template says it can do without. The refusal names the hook, the part and what it may use instead:

```
imap.hook.on-message-removed.notify.body: No such variable: $subject. This hook can use $id, $collection
```

Underneath it, a second failure of the same shape. A variable can be legitimate and still absent from one item: a message whose envelope carries no `From` leaves `$sender` unset, and an arrival whose resolution failed leaves every envelope name unset. Refusing those at load would be wrong, and dropping the notification over them was what happened. The runner now seeds the vocabulary empty and overwrites it with whatever resolved, so an absent value leaves a gap in the notification instead of taking the notification down.

Commands are not validated. Their placeholders reach them as environment variables, where an unset name is ordinary shell behaviour and any other shell variable is the script's own business.

Verified against a local Stalwart: an arrival naming `$sender` and `$subject` and a removal spelled `${subject:with no subject}` both fire, with no expansion failure in the log.

Capabilities moved: daemon.
