---
cairn: change
id: hook-template-variables
status: landed
created: 2026-08-22
---

# Refuse a template variable the hook cannot fill, and never drop a notification over one

## Why

A hook whose notification names a variable its event does not carry fires nothing and says nothing. `subst` refuses to expand a template with an unknown name, `fire_notification` hands back the failure, and the runner logs it at `warn` and moves on. The command half of the same hook runs, so the hook is half-fired, and the half that is missing is the visible one.

Found against a live Dovecot: an `on-message-removed` whose body reads `$subject`. The watch reports the removal, the hook resolves, and the notification never appears, leaving `notify hook failed: cannot expand body: No such variable: $subject` in a log nobody reads at that level. The configuration is wrong, and it was wrong when it was written: an expunged message has no envelope to read, so `$subject` on a removal is not a value that happens to be missing but one that can never exist.

That is the same failure the last change moved to load time everywhere else. The hooks a backend can fire are checked when the file is read; the variables those hooks can template against are not, though they are just as fixed.

There is a second, smaller failure underneath it. Even where a variable is legitimate, it may be absent from a given item: a message whose envelope carries no `From` leaves `$sender` unset, and an arrival whose resolution failed leaves every envelope name unset. Refusing those at load would be wrong, and dropping the notification over them is what happens today.

## What

- Each hook field declares the variables it can fill: `id` and `collection` everywhere, `flag` on a flag hook, and the envelope names (`subject`, `date`, `sender`, `sender_name`, `sender_address`, `recipient`, `recipient_name`, `recipient_address`) only on the IMAP arrival hook, that being the only one anything resolves an envelope for.
- Loading validates every `notify` summary and body against the vocabulary of the hook it hangs on, by expanding it against that vocabulary and nothing else. A name outside it is refused, naming the hook and what it may use instead. `${name:default}` keeps working, a default being how a template says it can do without.
- At runtime the vocabulary is seeded empty and then overwritten with what resolved, so a name that is legitimate but absent expands to nothing instead of taking the notification down with it.
- `cmd` is not validated. Its placeholders reach it as environment variables, where an unset name is ordinary shell behaviour and any other shell variable is fair game.
