# `lemonfiber playing`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber playing`

```text
Show what the media server is playing now: who is watching what, and where.

Every session in the house, or one member's with `--member`.

Usage: lemonfiber playing [OPTIONS]

Options:
      --json
          Print machine-readable output

      --member <MEMBER>
          Whose sessions, by the name they are known by

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
