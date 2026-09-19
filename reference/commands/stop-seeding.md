# `lemonfiber stop-seeding`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber stop-seeding`

```text
Stop seeding one completed download, and let its files go with it.

Everything the disk account names and deliberately leaves alone is asked for here instead, one download at a time. Named on its own it says what that download is, what it occupies, where it stands and, while it is still seeding, what removing it does to a private tracker's opinion of you. It removes nothing, and prints a name for that offer; answering with that name is the yes. There is no other way to say it, because a blanket confirmation would be agreement from somebody who had not read the cost.

Usage: lemonfiber stop-seeding [OPTIONS] <DOWNLOAD>

Arguments:
  <DOWNLOAD>
          The completed download, as the account and the client both name it

Options:
      --json
          Print machine-readable output

      --offer <NAME>
          The offer being answered, as the run that made it printed it

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
          Print help (see a summary with '-h')
```
