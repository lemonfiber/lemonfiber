# `lemonfiber up`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber up`

```text
Start a form, or the union of several

Usage: lemonfiber up [OPTIONS] [FORMS]...

Arguments:
  [FORMS]...
          The forms to start; none starts everything the stack declares

Options:
      --json
          Print machine-readable output

      --service <NAME>
          Start only these services, leaving the rest of the form alone

      --at-boot
          Start what a restart of this machine should start, and nothing otherwise.

          What a login runs. It brings back whichever form was last running unless you pinned one, and it declines — saying why — where you stopped the stack on purpose, where you never asked for it to start on its own, or where this machine is on its battery and you have not said to start anyway. It waits for the container engine to finish starting and tries again while the network is still arriving, and if the stack still does not come back it records that, so the next thing you type tells you once rather than not at all. Naming a form or a service alongside it is refused: which forms come back is the record's answer, not this command line's.

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
