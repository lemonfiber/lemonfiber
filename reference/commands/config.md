# `lemonfiber config`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber config`

```text
Read or change one setting

Usage: lemonfiber config [OPTIONS] <COMMAND>

Commands:
  get   Read one setting
  set   Change one setting
  show  Show every setting, with credentials withheld
  help  Print this message or the help of the given subcommand(s)

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

## `lemonfiber config get`

```text
Read one setting

Usage: lemonfiber config get [OPTIONS] <KEY>

Arguments:
  <KEY>
          The setting to read

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

## `lemonfiber config set`

```text
Change one setting.

Shows the difference between what the setting holds and what it would hold, and what changing it affects. A change setup catalogued as consequential — the data location, the download protocols, the user the services run as — is staged rather than applied until `--confirm`. A replacement credential is proven against its live service before the one it replaces is discarded.

Usage: lemonfiber config set [OPTIONS] <KEY> <VALUE>

Arguments:
  <KEY>
          The setting to change

  <VALUE>
          What to change it to

Options:
      --confirm
          Go ahead, having read what the change affects.

          Also stores a replacement credential that nothing could be reached to prove — for a machine that is offline, or a provider it cannot see, and writes over an edit made outside lemonfiber, or stops a download that is still coming down.

      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --wait
          Let anything still coming down finish before the change is made

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

## `lemonfiber config show`

```text
Show every setting, with credentials withheld

Usage: lemonfiber config show [OPTIONS]

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
