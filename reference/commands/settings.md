# `lemonfiber settings`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber settings`

```text
List every setting and secret the stack's services read, or change one setting.

Each setting with whose it is, every service that reads it and the variable it is read into, the default its manifest declares and your value where you recorded one. Each secret with the services that read it, where its value came from and whether one is held — never the value itself.

Usage: lemonfiber settings [OPTIONS] [COMMAND]

Commands:
  set    Record a value for one or more settings at once, regenerating every entry that reads them
  clear  Take your value away from one or more settings, so each declared default is written again
  help   Print this message or the help of the given subcommand(s)

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

## `lemonfiber settings set`

```text
Record a value for one or more settings at once, regenerating every entry that reads them.

The settings named are checked together, so settings whose choices depend on each other move in one act. Named on its own it says what the change would come to — the entries it regenerates and the running services it restarts — changes nothing, and prints a name for that offer; answering with that name is the yes.

Usage: lemonfiber settings set [OPTIONS] <SETTING=VALUE>...

Arguments:
  <SETTING=VALUE>...
          Each setting and its value as `SETTING=VALUE`, such as `gluetun:countries=Belgium`; the value is written exactly as given

Options:
      --json
          Print machine-readable output

      --offer <NAME>
          The offer being answered, as the run that made it printed it

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

## `lemonfiber settings clear`

```text
Take your value away from one or more settings, so each declared default is written again.

Named on its own it says what the change would come to and prints a name for that offer, as `set` does.

Usage: lemonfiber settings clear [OPTIONS] <SETTING>...

Arguments:
  <SETTING>...
          Each setting, such as `gluetun:countries`

Options:
      --json
          Print machine-readable output

      --offer <NAME>
          The offer being answered, as the run that made it printed it

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
