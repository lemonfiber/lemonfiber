# `lemonfiber reissue`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber reissue`

```text
Let somebody set a new password, without you choosing or seeing it.

Their account goes back to having no password on it — the state a fresh invitation leaves it in — so they claim it again by setting the first one themselves. Their old password stops working immediately. What this prints is the invitation to send them: the same address, the same code.

Usage: lemonfiber reissue [OPTIONS] <NAME>

Arguments:
  <NAME>
          Whose account to make claimable again

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
