# `lemonfiber trace`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber trace`

```text
Follow one show or film across the services — "where is my show?".

Searched for the way you would name it, not by an internal id. Reports how far it got and, where it plainly stopped, why. A show is reported season by season: how many episodes are here, and what each one that is not is waiting on.

Something monitored that has never been grabbed stopped for one of two reasons, and nothing on this machine can tell them apart: the indexers carry nothing for it, or they carry releases the quality you chose rejects. `--search` asks them.

Usage: lemonfiber trace [OPTIONS] <TERM>...

Arguments:
  <TERM>...
          The show or film to follow, named as you would say it

Options:
      --json
          Print machine-readable output

      --season <SEASON>
          Narrow to one season, instead of every season of the show

      --dry-run
          Say what would happen, and change nothing

      --search
          Ask the indexers what they carry, to tell "nothing at your quality" from "nothing at all". Spends one real search against their daily allowance

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
