# `lemonfiber undo`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber undo`

```text
Put back one run of changes, named by the stamp `lemonfiber history` shows.

The whole run and never half of one: a seed or a reconfigure is the unit an operator agreed to, and the history says beside each entry how many changes would go with it. Nothing is put back unless all of it can be.

Usage: lemonfiber undo [OPTIONS] <AT>

Arguments:
  <AT>
          The stamp of the run to put back, copied from `lemonfiber history`

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
