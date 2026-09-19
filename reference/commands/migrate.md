# `lemonfiber migrate`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber migrate`

```text
See what is already on this machine, and take it over if you choose to.

With nothing named it surveys: the stacks already standing here, the ports they hold that lemonfiber would want, and anything it could not take over as it stands. Nothing is started, stopped, moved, or written.

Usage: lemonfiber migrate [OPTIONS] [COMMAND]

Commands:
  adopt    Take over the setup already here, so lemonfiber manages it
  beside   Run alongside the setup already here, on ports nothing else is using
  import   Copy what the setup already here holds into lemonfiber's own services
  replace  Stand in place of the setup already here, stopping it and deleting none of it
  help     Print this message or the help of the given subcommand(s)

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

## `lemonfiber migrate adopt`

```text
Take over the setup already here, so lemonfiber manages it.

Without `--confirm` it says what adopting would come to and writes nothing — which databases a newer version would upgrade, and where their data sits so it can be backed up first.

Usage: lemonfiber migrate adopt [OPTIONS]

Options:
      --confirm
          Go ahead, having backed up the data named

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

## `lemonfiber migrate beside`

```text
Run alongside the setup already here, on ports nothing else is using.

Without `--confirm` it says where each service would listen and writes nothing. Nothing of the existing setup is touched either way.

Usage: lemonfiber migrate beside [OPTIONS]

Options:
      --confirm
          Go ahead, having seen where each service would listen

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

## `lemonfiber migrate import`

```text
Copy what the setup already here holds into lemonfiber's own services.

Without `--confirm` it names what it would carry and carries nothing. Nothing of the existing setup is written to either way.

Usage: lemonfiber migrate import [OPTIONS]

Options:
      --confirm
          Go ahead, having seen what would be carried

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

## `lemonfiber migrate replace`

```text
Stand in place of the setup already here, stopping it and deleting none of it.

Without `--confirm` it names what it would stop and stops nothing. Nothing is deleted either way, so the old stack can be started again.

Usage: lemonfiber migrate replace [OPTIONS]

Options:
      --confirm
          Go ahead, having seen what would stop

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
