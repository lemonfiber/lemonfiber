# `lemonfiber outbound`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber outbound`

```text
List everything that leaves this machine, and what refusing each of it costs.

lemonfiber's own requests first — where each goes, why, exactly what travels, whether it is on, the setting that switches it off and what stops working when it is — then the requests the stack's own services make, which are theirs rather than lemonfiber's.

Usage: lemonfiber outbound [OPTIONS]

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
