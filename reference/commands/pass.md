# `lemonfiber pass`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber pass`

```text
Confirm live-stream passes for the stream guard, for as long as it runs.

Run by the stack as a service of its own, holding the pass key read-only, and never by hand.

Usage: lemonfiber pass [OPTIONS] --listen <ADDRESS>

Options:
      --config <PATH>
          The directory holding the pass key and the media server's live-stream paths

          [default: /config]

      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --listen <ADDRESS>
          The address to answer at: the service's own on the network it shares with the guard

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
