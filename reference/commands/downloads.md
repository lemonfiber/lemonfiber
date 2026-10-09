# `lemonfiber downloads`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber downloads`

```text
Pause every download client the stack runs, or let them all fetch again

Usage: lemonfiber downloads [OPTIONS] <COMMAND>

Commands:
  pause   Stop every download client fetching, until you resume them
  resume  Let every download client fetch again
  help    Print this message or the help of the given subcommand(s)

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --offer <NAME>
          The offer being answered, as the rehearsal printed it. Carrying one, no client is told anything where the clients, or what each said it was doing, have moved

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help
```

## `lemonfiber downloads pause`

```text
Stop every download client fetching, until you resume them.

Each client is named with what it said afterwards, so one that went on fetching is named as fetching. A pause holds until you resume it: a schedule, a new month or a lifted cap does not undo it.

Usage: lemonfiber downloads pause [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --offer <NAME>
          The offer being answered, as the rehearsal printed it. Carrying one, no client is told anything where the clients, or what each said it was doing, have moved

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber downloads resume`

```text
Let every download client fetch again

Usage: lemonfiber downloads resume [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --offer <NAME>
          The offer being answered, as the rehearsal printed it. Carrying one, no client is told anything where the clients, or what each said it was doing, have moved

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help
```
