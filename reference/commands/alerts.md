# `lemonfiber alerts`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber alerts`

```text
Choose how much lemonfiber tells you about, in plain language

Usage: lemonfiber alerts [OPTIONS] <COMMAND>

Commands:
  show  Show what you are told about, and what else you could be
  set   Choose how much to be told
  help  Print this message or the help of the given subcommand(s)

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

## `lemonfiber alerts show`

```text
Show what you are told about, and what else you could be

Usage: lemonfiber alerts show [OPTIONS]

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

## `lemonfiber alerts set`

```text
Choose how much to be told

Usage: lemonfiber alerts set [OPTIONS] <PRESET>

Arguments:
  <PRESET>
          How much to hear: problems-only, with-completions, or everything

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
