# `lemonfiber ps`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber ps`

```text
Report what each service is actually doing

Usage: lemonfiber ps [OPTIONS] [FORMS]...

Arguments:
  [FORMS]...
          The forms to report on; none reports the whole stack

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
