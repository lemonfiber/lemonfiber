# `lemonfiber switch`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber switch`

```text
Make these forms the active set, leaving shared services running.

Only what falls outside the new shape is stopped. A service the old shape and the new one both hold keeps running rather than being restarted, so a download in flight is not interrupted to change the stack around it.

Usage: lemonfiber switch [OPTIONS] <FORMS>...

Arguments:
  <FORMS>...
          The forms to switch to

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
