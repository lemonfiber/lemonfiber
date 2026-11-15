# `lemonfiber guide`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber guide`

```text
Show the live TV guide one member may watch: each channel's programmes from now.

Read as that member across the hours `LEMONFIBER_GUIDE_HOURS` holds: a channel whose programme now is outside their limits is left out, a programme outside them is left out as a gap, and each channel says where the media server's guide ends.

Usage: lemonfiber guide [OPTIONS]

Options:
      --json
          Print machine-readable output

      --member <MEMBER>
          Whose guide, named the way they sign in

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
