# `lemonfiber play`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber play`

```text
Say where one title plays in a member's browser, on a pass holding a session on their own account.

The pass lapses five minutes after it is issued; hand it back with `--pass` before then to keep playing. The location is also drawn as a code to point a camera at.

Usage: lemonfiber play [OPTIONS] <ITEM>

Arguments:
  <ITEM>
          The title, by the id the shelf lists it under

Options:
      --json
          Print machine-readable output

      --member <MEMBER>
          Who it plays as, named the way they sign in

      --device <DEVICE>
          The id the browser keeps for itself: eight to sixty-four letters, digits and dashes

      --pass <PASS>
          The pass the browser holds, kept for longer around the same session

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
