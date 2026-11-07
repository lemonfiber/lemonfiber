# `lemonfiber push-endpoint`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber push-endpoint`

```text
List or remove the phones that asked to be woken when this stack has something to say

Usage: lemonfiber push-endpoint [OPTIONS] <COMMAND>

Commands:
  list    List every push endpoint by name, address and when it was registered
  remove  Remove a push endpoint by name. Its phone is not woken from then on
  help    Print this message or the help of the given subcommand(s)

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

## `lemonfiber push-endpoint list`

```text
List every push endpoint by name, address and when it was registered

Usage: lemonfiber push-endpoint list [OPTIONS]

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

## `lemonfiber push-endpoint remove`

```text
Remove a push endpoint by name. Its phone is not woken from then on

Usage: lemonfiber push-endpoint remove [OPTIONS] <NAME>

Arguments:
  <NAME>
          The name it was registered under

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
