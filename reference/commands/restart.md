# `lemonfiber restart`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber restart`

```text
Restart services without touching the rest

Usage: lemonfiber restart [OPTIONS] <FORM> [SERVICES]...

Arguments:
  <FORM>
          The form holding them

  [SERVICES]...
          The services to restart; none restarts the whole form

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
