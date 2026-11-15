---
cairn: change
id: wizard
status: landed
created: 2026-08-22
---

# Generate a first account from an email address

## Why

carillon is the only Pimalaya CLI left without a wizard. himalaya, comodoro and ortie all meet a newcomer the same way: run the binary, get a welcome naming the file that is missing, answer a few prompts, and end with a working account written where the loader reads it. carillon instead says "copy config.sample.toml and delete the rest", which asks someone to learn a five-backend configuration surface before their first notification ever fires.

The information the sample asks for is exactly what discovery already knows: io-pim-discovery turns an email address into the IMAP, JMAP, CalDAV and CardDAV endpoints of a provider, and the DAV home-set walk turns a credential into the list of calendars and addressbooks that account holds. What is left to ask is which service to watch and how to authenticate against it.

## What

The himalaya wizard system, verbatim where it can be: same trigger, same prompts, same ending.

- `carillon configure` (alias `wizard`) runs it, and a bare `carillon` offers it when no configuration is found, welcome first. A command that needs an account raises the same offer instead of the current "copy the sample" bail.
- One prompt takes an email address (or a bare domain, a `scheme://` URL, or a local folder path for Maildir). Discovery runs behind a spinner and every reachable service becomes one selectable entry.
- The chosen service prompts its authentication method among those advertised, then its credentials through the shared pimalaya-cli keyring picker (OS keyrings, OAuth brokers, custom command, raw value).
- The connection is tested before anything is written, and the test is also what fills in what carillon needs and discovery does not carry: the DAV collection to watch, chosen from the calendars or addressbooks the home-set holds, and the components a calendar advertises.
- The watch method is not prompted. carillon already knows the best one per backend, and the account writes it only when the server cannot do it: an IMAP server not advertising IDLE, or a JMAP session with no event-source URL, both fall back to an explicit poll. Everything else leaves `watch` unset, which is the best method by definition.
- The account ends as a file to create, a block to append, or a document on stdout, exactly as in himalaya and comodoro: same prompts, same rules on the account name and the `default` flag, same plain-text append that leaves comments and formatting untouched.
- `--help` gains the shared `footer!()` (bug tracker, sponsoring), which carillon is also alone in missing.

A generated account carries one notify hook on the arrival event of its backend (per component held, for a calendar). A watch with no hook is a watch that does nothing, and the wizard's job is an account that works when it is saved.

## What it forces

The account has to render back as TOML. The config structs serialize already, but the defaults come out with them, so the fields that carry one gain a `skip_serializing_if` and `AccountConfig` gains the `render` himalaya has: group by backend, endpoint first, one blank line between groups.

`Config::load` currently bails on a missing file. It becomes the plain loader, and the offer moves up to the CLI, which is where the printer and the terminal test live.

## Not done

- Gmail and Microsoft Graph short-circuits: carillon has no proprietary backend, so a Google or Microsoft account is configured over IMAP or DAV like any other.
- Hooks beyond the arrival one, flag filters, notification templates: prompting for those would be a worse text editor than the one the user already has.
- SMTP, which carillon does not speak.
