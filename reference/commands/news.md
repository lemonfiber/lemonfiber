# `lemonfiber news`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber news`

```text
List lemonfiber's releases, what the household has asked for, and the checks found wrong, each newest first.

Each carries what names it, so a screen that remembers the newest it has shown can mark what came after. The stack keeps nothing of what anybody has seen.

Usage: lemonfiber news [OPTIONS]

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
