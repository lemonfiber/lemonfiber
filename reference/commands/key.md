# `lemonfiber key`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber key`

```text
Mint, list or revoke the keys another program reaches the web interface with

Usage: lemonfiber key [OPTIONS] <COMMAND>

Commands:
  mint    Mint a key for a program, under a name and with one scope
  list    List every key by name, scope, purpose, state, minting time and last use
  revoke  Revoke a key by name. It is refused from its next request
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

## `lemonfiber key mint`

```text
Mint a key for a program, under a name and with one scope.

Its secret is shown once, here, beside the address and the certificate pin a program on another machine needs. Nothing keeps the secret: lose it and mint another. Every mint is journaled and raises an alert.

Usage: lemonfiber key mint [OPTIONS] --scope <SCOPE> --purpose <PURPOSE> <NAME>

Arguments:
  <NAME>
          What to call it, as in home-assistant

Options:
      --json
          Print machine-readable output

      --scope <SCOPE>
          What it admits: read, act or member:<account>

      --dry-run
          Say what would happen, and change nothing

      --purpose <PURPOSE>
          What it is for: home-assistant, mcp or other

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

## `lemonfiber key list`

```text
List every key by name, scope, purpose, state, minting time and last use

Usage: lemonfiber key list [OPTIONS]

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

## `lemonfiber key revoke`

```text
Revoke a key by name. It is refused from its next request

Usage: lemonfiber key revoke [OPTIONS] <NAME>

Arguments:
  <NAME>
          The key's name, as the listing shows it

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
