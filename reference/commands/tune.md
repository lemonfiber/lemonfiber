# `lemonfiber tune`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber tune`

```text
Open a channel's live stream for one member, and say where it is served.

Each location carries a pass for that live stream and that member, which lapses five minutes after it is issued. Ask again with `--stream` before then to keep it, and with `--stop` as well to close it. Every location is also drawn as a code to point a camera at.

Usage: lemonfiber tune [OPTIONS] [CHANNEL]

Arguments:
  [CHANNEL]
          The channel, by the id the channels list gives it

Options:
      --json
          Print machine-readable output

      --member <MEMBER>
          Who watches it, named the way they sign in

      --stream <STREAM>
          The live stream to keep or close, as an earlier answer named it

      --stop
          Close the live stream named rather than keep it

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
