# `lemonfiber channels`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber channels`

```text
Show the live TV channels one member may watch, each with what is on it now.

Read as that member: a channel whose programme now is outside their limits is left out, and a member whose account may not watch live TV is shown none.

Usage: lemonfiber channels [OPTIONS]

Options:
      --json
          Print machine-readable output

      --member <MEMBER>
          Whose channels, named the way they sign in

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
