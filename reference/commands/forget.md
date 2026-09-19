# `lemonfiber forget`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber forget`

```text
Remove everything lemonfiber keeps on this machine.

The two directories and everything under them. Your library, your downloads and the containers are not lemonfiber's and are never touched. Because it throws work away it lists what would go and does nothing until `--confirm`.

Usage: lemonfiber forget [OPTIONS]

Options:
      --confirm
          Go ahead and remove it, having seen what would go

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
