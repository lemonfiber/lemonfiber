# `lemonfiber reset`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber reset`

```text
Put the stack back to lemonfiber's own state, reverting every edit you made.

The opposite of adopt: it discards your hand-edits to the stack files and restores lemonfiber's own. Because it throws work away, it names exactly what will be lost, does nothing, and prints a name for that offer — run it once to see the diffs, again with `--offer` and that name to reset.

Usage: lemonfiber reset [OPTIONS]

Options:
      --json
          Print machine-readable output

      --offer <NAME>
          The offer being answered, as the reading that made it printed it

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
