# `lemonfiber` — command reference

Generated from the command line's own declarations. Run `just reference` to rewrite it.

## `lemonfiber`

```text
Orchestrates a fully open-source, self-hosted media stack

Usage: lemonfiber [OPTIONS] [COMMAND]

Commands:
  setup         Set up the stack by answering a few questions
  version       Report the versions in play
  forms         List the forms this stack has, and what each one is for
  migrate       See what is already on this machine, and take it over if you choose to
  up            Start a form, or the union of several
  down          Stop and remove what a form started
  switch        Make these forms the active set, leaving shared services running
  restart       Restart services without touching the rest
  pull          Fetch newer images without applying them
  ps            Report what each service is actually doing
  logs          Show what services are saying
  config        Read or change one setting
  alerts        Choose how much lemonfiber tells you about, in plain language
  quality       Choose how good your media should look, in plain language
  doctor        Run the checks that prove the stack is doing what it should
  plugin        Read what a plugin may claim, where it may contribute, and how it is written
  watch         Guard the data location while forms run, stopping them if it disappears
  hosting       Say what this machine keeps running when no terminal is open
  trace         Follow one show or film across the services — "where is my show?"
  household     Show who is in the household, what each may watch and ask for, and what each asked for
  walkthrough   Add one thing, end to end, and watch every step of it happen
  explain       Say what one of this product's words means
  history       Show everything lemonfiber changed, newest first, and how far each could be put back
  undo          Put back one run of changes, named by the stamp `lemonfiber history` shows
  stuck         List the items whose downloads are stuck — the landing point for "N stuck", each named so `lemonfiber trace` follows it on its own
  front-door    Name the one address to send somebody who lives here
  catalogue     Say what each service in this stack is for, and what became of any it dropped
  outbound      List everything that leaves this machine, and what refusing each of it costs
  provenance    Say where each service comes from: its licence, its project, and the exact version this stack pins it at
  credentials   Say which credentials this stack holds, or act on one of them
  stored        List what lemonfiber keeps on this machine, where it is, and why
  clients       Say which app to watch on, for each kind of device somebody in the house has
  invite        Offer somebody in the house an account they can claim
  reissue       Let somebody set a new password, without you choosing or seeing it
  remove        Take somebody out of the household, in both places they have an account
  forget        Remove everything lemonfiber keeps on this machine
  uninstall     Take lemonfiber off this machine, at one of four removals
  space         Account for the disk: where the room went, when it runs out, what can go
  stop-seeding  Stop seeding one completed download, and let its files go with it
  bandwidth     Account for the line: what it carries, what the stack takes, what that costs
  seed          Wire the stack's services to each other, idempotently
  adopt         Adopt your current edits as lemonfiber's expected state
  reset         Put the stack back to lemonfiber's own state, reverting every edit you made
  update        Move something onto a newer version, naming which
  backup        Back up your configuration to an archive, so it stops being precious
  support       Gather everything a person helping you would ask for, with every value not named safe replaced by a stand-in
  ui            Serve the web interface, for as long as you leave it running
  restore       Restore your configuration from a backup archive
  help          Print this message or the help of the given subcommand(s)

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help

  -V, --version
          Print version
```

## Every command

- [`lemonfiber setup`](commands/setup.md)
- [`lemonfiber version`](commands/version.md)
- [`lemonfiber forms`](commands/forms.md)
- [`lemonfiber migrate`](commands/migrate.md)
- [`lemonfiber up`](commands/up.md)
- [`lemonfiber down`](commands/down.md)
- [`lemonfiber switch`](commands/switch.md)
- [`lemonfiber restart`](commands/restart.md)
- [`lemonfiber pull`](commands/pull.md)
- [`lemonfiber ps`](commands/ps.md)
- [`lemonfiber logs`](commands/logs.md)
- [`lemonfiber config`](commands/config.md)
- [`lemonfiber alerts`](commands/alerts.md)
- [`lemonfiber quality`](commands/quality.md)
- [`lemonfiber doctor`](commands/doctor.md)
- [`lemonfiber plugin`](commands/plugin.md)
- [`lemonfiber watch`](commands/watch.md)
- [`lemonfiber hosting`](commands/hosting.md)
- [`lemonfiber trace`](commands/trace.md)
- [`lemonfiber household`](commands/household.md)
- [`lemonfiber walkthrough`](commands/walkthrough.md)
- [`lemonfiber explain`](commands/explain.md)
- [`lemonfiber history`](commands/history.md)
- [`lemonfiber undo`](commands/undo.md)
- [`lemonfiber stuck`](commands/stuck.md)
- [`lemonfiber front-door`](commands/front-door.md)
- [`lemonfiber catalogue`](commands/catalogue.md)
- [`lemonfiber outbound`](commands/outbound.md)
- [`lemonfiber provenance`](commands/provenance.md)
- [`lemonfiber credentials`](commands/credentials.md)
- [`lemonfiber stored`](commands/stored.md)
- [`lemonfiber clients`](commands/clients.md)
- [`lemonfiber invite`](commands/invite.md)
- [`lemonfiber reissue`](commands/reissue.md)
- [`lemonfiber remove`](commands/remove.md)
- [`lemonfiber forget`](commands/forget.md)
- [`lemonfiber uninstall`](commands/uninstall.md)
- [`lemonfiber space`](commands/space.md)
- [`lemonfiber stop-seeding`](commands/stop-seeding.md)
- [`lemonfiber bandwidth`](commands/bandwidth.md)
- [`lemonfiber seed`](commands/seed.md)
- [`lemonfiber adopt`](commands/adopt.md)
- [`lemonfiber reset`](commands/reset.md)
- [`lemonfiber update`](commands/update.md)
- [`lemonfiber backup`](commands/backup.md)
- [`lemonfiber support`](commands/support.md)
- [`lemonfiber ui`](commands/ui.md)
- [`lemonfiber restore`](commands/restore.md)
