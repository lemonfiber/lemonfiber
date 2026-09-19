# `lemonfiber remove`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber remove`

```text
Take somebody out of the household, in both places they have an account.

Revokes access to the media server and to the request service. Their watch history goes with the account — the media server offers no way to keep it — and the request service destroys what they asked for. Because none of that can be got back, it says what would go and does nothing until `--confirm`.

Usage: lemonfiber remove [OPTIONS] <NAME>

Arguments:
  <NAME>
          Whose account to take away, as `lemonfiber household` shows them

Options:
      --confirm
          Go ahead and remove them, having seen what goes

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
