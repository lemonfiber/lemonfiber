# `lemonfiber adopt`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber adopt`

```text
Adopt your current edits as lemonfiber's expected state.

A value you changed by hand reports as drift until you adopt it; once adopted it is kept across future seeds and restores. Wires what is missing as a seed does, and promotes every drifted value to yours.

Usage: lemonfiber adopt [OPTIONS]

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
