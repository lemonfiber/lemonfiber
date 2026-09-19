# `lemonfiber backup`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber backup`

```text
Back up your configuration to an archive, so it stops being precious

Usage: lemonfiber backup [OPTIONS]

Options:
      --json
          Print machine-readable output

      --service <SERVICE>
          Back up one service's configuration instead of the whole stack

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
