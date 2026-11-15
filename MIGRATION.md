# Migration guide

One section per major version, the newest first: what to change in a configuration, a script and a JSON consumer when upgrading. The full configuration schema lives in [config.sample.toml](./config.sample.toml), and the exhaustive list of changes in [CHANGELOG.md](./CHANGELOG.md); these are the ones to act on.

## From 0.2 to 1.0

1.0 brings Carillon onto the standard the Pimalaya CLIs share: no wizard, a configuration refusing what it does not read, hooks shaped as Comodoro's, one path per `-c`, and a failure contract (exit codes and stable error codes) a script can rely on. A 0.2 configuration does not load as it is: its hooks at least need renaming. Every refusal names the key, its line and what replaced it, so running `carillon check` after each edit walks you through the file.

### Hooks

- **The hook table is `hooks`**, under each backend, where 0.2 read `hook` (and `hooks` as an alias): `imap.hooks`, `caldav.hooks`, `msgraph.hooks` and so on. `<backend>.hook` is refused, naming `hooks` and this page.
- **A hook's command is `command`**, where 0.2 read `cmd`. A string still goes through the platform shell and a list is still spawned directly. `cmd` still reads, as the alias of `command` wherever the family configures a command (a secret's `password.cmd` included), so a 0.2 hook needs no change there; write `command` in a new one.
- **Every event name ends in a bare verb**: `-add`, `-remove` and `-change` where 0.2 read `-added`, `-removed` and `-changed`. The former names are refused, each naming its successor. `on-alarm-trigger` keeps its name.

  | 0.2 | 1.0 |
  | --- | --- |
  | `on-message-added`, `on-message-removed`, `on-message-changed` | `on-message-add`, `on-message-remove`, `on-message-change` |
  | `on-flag-added`, `on-flag-removed` | `on-flag-add`, `on-flag-remove` |
  | `on-card-added`, `on-card-removed`, `on-card-changed` | `on-card-add`, `on-card-remove`, `on-card-change` |
  | `on-event-added`, `on-event-removed`, `on-event-changed` | `on-event-add`, `on-event-remove`, `on-event-change` |
  | `on-task-added`, `on-task-removed`, `on-task-changed` | `on-task-add`, `on-task-remove`, `on-task-change` |

- **Templates use the shell's syntax**: a default is `${name:-default}` where 0.2 wrote `${name:default}`, and `${name:default}` is refused naming it. A default takes over when the value is empty.
- **The envelope variables are camelCase**, in notifications and in the environment of commands alike: `$senderName`, `$senderAddress`, `$recipientName` and `$recipientAddress` where 0.2 had `$sender_name`, `$sender_address`, `$recipient_name` and `$recipient_address`. A notification naming a former spelling is refused naming it; a command reading `$sender_name` from its environment now reads an unset variable, so rename it in your scripts too. `$id`, `$subject`, `$date`, `$sender`, `$recipient`, `$flag` and the collection names are unchanged, and every hook gains `$collection`.
- **A default no longer excuses a variable the hook does not have**: a removal reading `${subject:with no subject}` loaded in 0.2 and is refused now, an expunged message having no envelope. Drop the variable from that hook.

A 0.2 hook and its 1.0 spelling:

```toml
# 0.2
imap.hook.on-message-added.notify.summary = "New mail from $sender_name"
imap.hook.on-message-added.cmd = 'echo "$id from $sender_address" >> ~/mail.log'
imap.hook.on-message-removed.notify.summary = "Mail ${subject:with no subject} deleted"
imap.hook.on-flag-added.flags = ["Seen"]
imap.hook.on-flag-added.cmd = ["notify-send", "$flag set on $id"]
caldav.hook.on-event-changed.notify.summary = "${calendar:work} changed"

# 1.0
imap.hooks.on-message-add.notify.summary = "New mail from $senderName"
imap.hooks.on-message-add.command = 'echo "$id from $senderAddress" >> ~/mail.log'
imap.hooks.on-message-remove.notify.summary = "Mail $id deleted"
imap.hooks.on-flag-add.flags = ["Seen"]
imap.hooks.on-flag-add.command = ["notify-send", "$flag set on $id"]
caldav.hooks.on-event-change.notify.summary = "${calendar:-work} changed"
```

The renames are mechanical, so a 0.2 file can be moved over with one command (check the result, then fix the defaults and the removal hooks by hand):

```sh
perl -pi -e 's/\.hook\./.hooks./; s/\b(on-[a-z]+-(?:add|remov|chang))ed\b/$1 =~ s{(remov|chang)$}{$1e}r/ge; s/(on-[a-z]+-[a-z]+)\.cmd\b/$1.command/; s/\$\{(\w+):(?!-)/\${$1:-/g; s/\b(sender|recipient)_(name|address)\b/$1\u$2/g' ~/.config/carillon/config.toml
```

### Configuration and first run

- **The wizard is gone**: `carillon configure` (alias `wizard`) and its `carillon-configure` schema no longer exist, and no command offers to write a configuration. An existing configuration keeps working once migrated; a new one is written by hand from [config.sample.toml](./config.sample.toml): copy it to $XDG_CONFIG_HOME/carillon/config.toml, keep one backend and the hooks you want, delete the rest, then run `carillon check`. Run at a terminal with no configuration, any command prints the commands doing the copy, a bare `carillon` included. A bare `carillon` prints the help once a configuration exists.
- **An unknown key is refused at every level**, the root and the account included, naming the key and its line, where 0.2 ignored it there: fix a misspelt key, and delete a key another tool reads (an account shared with himalaya goes in its own file). A backend the build leaves out is no unknown key: its block loads, and `watch` or `check` reaching it fails coded `backend-not-built`, naming the cargo feature to build with, where 0.2 skipped it silently. `watch` stops that account for the run and watches the others, and fails, exit 1, when no account is left. A key carillon moved under each backend before 0.1 is refused naming where it went, and was silently ignored until now:
  - `folder`, `mailbox` and `collection` on the account: name the collection under its backend, `imap.mailbox`, `maildir.mailbox`, `caldav.calendar`, `carddav.addressbook` and so on.
  - `watch` on the account: move it under its backend, `imap.watch.poll.interval` for instance.
  - `hooks` on the account: move each hook under its backend as `<backend>.hooks`, `hooks.on-message-added` becoming `imap.hooks.on-message-add`. Those hooks never fired while the table sat on the account.
  - `dav`: watch a calendar with a `caldav` block or an addressbook with a `carddav` block.
- **Several accounts marked default are refused**, naming them, where one of them was picked: keep `default = true` on one.

```toml
# 0.2: ignored keys, an account-level hook table, two defaults
[accounts.perso]
default = true
mailbox = "INBOX"
hooks.on-message-added.cmd = "true"
imap.server = "imap.example.org"

[accounts.work]
default = true
imap.mailbox = "INBOX"

# 1.0
[accounts.perso]
default = true
imap.mailbox = "INBOX"
imap.server = "imap.example.org"
imap.hooks.on-message-add.command = "true"

[accounts.work]
imap.mailbox = "INBOX"
```

- **A calendar or an addressbook named by one path segment** (`caldav.calendar = "work"`, `carddav.addressbook = "default"`) is looked up under the home set the server's principal names, which is the id a listing of the account gives, where 0.2 read it under `server`. An absolute path (`/dav/calendars/alice/work/`) is read as it stands: write one if the segment named a collection outside the home set.
- **`gpeople.addressbook = "contacts"` watches every contact the account owns**, where it named a contact group called `contacts`, and `msgraph.addressbook = "contacts"` names the default contact folder first.
- **A pimdir watch reads pimdir draft-11 stores**, the ones neverest writes after v0.3.0 (io-pimdir 0.12). A store an earlier neverest wrote is refused (`store-version`): reinstall it with the current neverest, as [its migration guide](https://github.com/pimalaya/neverest/blob/master/MIGRATION.md) describes. The `pimdir` backend is new since 0.2, so this concerns a git build only.

### Command line and scripts

- **`-c` takes one path per flag**: a value is no longer split on `:`. Repeat the flag to merge several files, the first one being the base.
- **`CARILLON_CONFIG` is no longer read**: pass the path with `-c` instead, for example in the systemd unit or the script starting the watch. Left in the environment, the variable is ignored and the default paths are read.
- **`-a default` names the account called `default`**, like any other name, where 0.2's `check` took the account marked default: to take that one, omit `-a` (`carillon check`). With no account called `default`, `-a default` fails coded `account-missing`, saying to omit `-a`. `check` naming an unknown account lists the ones the configuration holds, as `watch` does.

```sh
# 0.2
CARILLON_CONFIG=~/.config/carillon/base.toml:~/.config/carillon/secrets.toml carillon watch
carillon -c base.toml:secrets.toml watch
# 1.0
carillon -c ~/.config/carillon/base.toml -c ~/.config/carillon/secrets.toml watch
carillon -c base.toml -c secrets.toml watch
```

- **A bare `carillon` with no configuration fails** as any command does, exit 1 naming the path it looked at and the sample, where it printed the help and exited 0 (0.2 offered the wizard there). A script running `carillon` to test the installation reads 1 until a configuration exists: run `carillon --help` or `carillon --version` instead.
- **`manual` and `completion` given no argument fail**, exit 1 with an error saying to use `--dir` on stdout (under `--json`, the failure object), where they printed their help on stderr and exited 2. Both still print one page or one script to stdout when given its name, and `--dir` writes them all.

```sh
# 0.2
carillon manual      # the help of `manual` on stderr, exit 2
# 1.0
carillon manual      # Error: Writing several manual pages to the standard output is ambiguous, use --dir to generate them as files (exit 1)
carillon manual --dir ~/.local/share/man/man1
carillon completion bash > ~/.local/share/bash-completion/completions/carillon
```

- **Exit codes**: 0 is a success, 1 a failure of the command itself (no configuration, an unknown account, a configuration refused), 2 a command line carillon refused (an unknown flag or command, a missing argument), and 3 a `check` that printed its report with a failed backend, where 0.2 exited 0 there. A script reading the exit status of `carillon check` now sees the failure.

```sh
# 0.2: check exited 0 whatever its rows said
carillon check && systemctl --user start carillon
# 1.0: 3 means the report printed and a backend failed
carillon check
case $? in
  0) systemctl --user start carillon ;;
  3) echo "a backend failed, see the report above" >&2 ;;
  *) echo "carillon could not run the check" >&2 ;;
esac
```

- **`watch` prints what it observes on stdout**, one line per event, then one result line once SIGINT, SIGTERM or SIGHUP stops it, exiting 0; 0.2 printed nothing there but its Ctrl+C notice. A unit or a script redirecting its stdout now receives these lines: send them to `/dev/null` if nothing reads them, or read them instead of scraping the logs. A lost session and a failed hook are events now rather than `warn` log lines, and a hook command exiting non-zero counts as a failed hook. **A hook command's output goes to stderr**: what a hook printed on stdout, which reached carillon's stdout in 0.2, is now on carillon's stderr beside the logs, so redirect `2>` to keep it (`carillon watch 2>>~/hooks.log`). A second signal ends the process at once, with 130.

```sh
# 0.2: nothing on stdout
carillon watch
# 1.0: one line per event, then the result once Ctrl+C is pressed
carillon watch
# [perso] watching mailbox `INBOX` over imap, idling
# [perso] mailbox `INBOX`: message 42 added
# Watch stopped by a signal
```

### JSON consumers

- **Failures carry a stable code**: under `--json`, the failure object gains `code` beside `error`, `sources` and `backtrace`, `code` being `null` for a failure carrying none, and each `check` row reads `{backend, ok, code, error}`. Match on `code` (`permission-denied`, `quota-exceeded`, `rate-limited`, `mailbox-not-found`, `calendar-not-found`, `addressbook-not-found`, `store-version` and the other pimdir store codes, `account-missing`, `account-default-missing`, `account-default-several`, `backend-not-built`, `usage-invalid`) rather than on the wording of `error`, which may change. `carillon json-schema carillon-error` describes the failure object.
- **A command line refused under `--json`** prints its failure object, coded `usage-invalid`, on stdout and nothing on stderr, still exiting 2, where 0.2 printed clap's message on stderr.
- **`watch --json` prints events, then one result**: each event is an object carrying `event` (`session-opened`, `session-lost`, `changed`, `hook-failed`, `alarm-fired`) and `account`, the result `{"stoppedBy":"signal"}` carries none. `carillon json-schema carillon-watch.events` describes the events, `carillon-watch` the result.
- **Piped `--json` output prints one value per line**, and each schema of `carillon json-schema` carries an `$id` naming the contract version, `https://pimalaya.org/schemas/carillon/v1/<name>.json` from 1.0 on.

```jsonc
// 0.2: carillon --json check, exiting 0
{"account":"perso","backends":[{"backend":"imap","ok":false,"error":"…"}]}
// 1.0, exiting 3
{"account":"perso","backends":[{"backend":"imap","ok":false,"code":"permission-denied","error":"…"}]}

// 0.2: carillon --json -a wrok check, exiting 1
{"error":"…","sources":[],"backtrace":null}
// 1.0, exiting 1
{"code":"account-missing","error":"Account `wrok` not found, the configuration holds: perso, work","sources":[],"backtrace":null}

// 1.0: carillon --json watch --nope, exiting 2, where 0.2 printed clap's message on stderr
{"code":"usage-invalid","error":"unexpected argument '--nope' found\n\nUsage: carillon watch [OPTIONS]\n\nFor more information, try '--help'.","sources":[],"backtrace":null}

// 1.0: carillon --json watch, then SIGTERM, exiting 0, where 0.2 printed nothing
{"event":"session-opened","account":"perso","backend":"imap","method":"idle","collections":[{"name":"mailbox","value":"INBOX"}]}
{"event":"changed","account":"perso","collection":{"name":"mailbox","value":"INBOX"},"change":{"kind":"item-added","domain":"message","id":"42"}}
{"event":"hook-failed","account":"perso","reaction":"command","code":null,"error":"Hook command exited with exit status: 3","sources":[]}
{"stoppedBy":"signal"}
```

## From 0.1 to 0.2

Nothing to change: a 0.1 configuration loads as it is, every key it read is still read, and the new backends and keys are additions.

- **`completions` and `manuals` became `completion` and `manual`**, the plurals staying as hidden aliases: a script calling them keeps working, but write the singular.
