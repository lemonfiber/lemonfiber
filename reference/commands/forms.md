# `lemonfiber forms`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber forms`

```text
List the forms this stack has, and what each one is for.

A form says which part of the stack to run. They come from the stack rather than from lemonfiber, so a stack of your own names its own.

Naming one says what starting it would come to — the services it holds, and anything your configuration leaves out — without starting anything.

Usage: lemonfiber forms [OPTIONS] [FORMS]...

Arguments:
  [FORMS]...
          The forms to describe; none lists them all

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
