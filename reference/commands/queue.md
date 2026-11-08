# `lemonfiber queue`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber queue`

```text
Say what the download queue holds that is wedged, why, how many strikes each has, and what the newest runs did to it

Usage: lemonfiber queue [OPTIONS] [COMMAND]

Commands:
  heal  Look at the queue now, and act on every download that has struck out where healing is on: its curator drops it, blocklists its release and searches for it again
  help  Print this message or the help of the given subcommand(s)

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
```

## `lemonfiber queue heal`

```text
Look at the queue now, and act on every download that has struck out where healing is on: its curator drops it, blocklists its release and searches for it again.

With `--dry-run`, says what it would do to each and keeps that run for `lemonfiber queue` to show, touching nothing else.

Usage: lemonfiber queue heal [OPTIONS]

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
          Print help (see a summary with '-h')
```
