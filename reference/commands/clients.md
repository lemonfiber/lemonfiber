# `lemonfiber clients`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber clients`

```text
Say which app to watch on, for each kind of device somebody in the house has.

The client landscape is uneven and it matters which app is used: some devices have an official one that works, and a smart television may have nothing worth using. This says which is which, names a browser as the answer that always works and needs no installation, and where a device is badly served says what to do instead rather than leaving somebody to find out by failing.

Usage: lemonfiber clients [OPTIONS]

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
