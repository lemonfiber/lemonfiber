# `lemonfiber logs`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber logs`

```text
Show what services are saying

Usage: lemonfiber logs [OPTIONS] [SERVICES]...

Arguments:
  [SERVICES]...
          The services to read; none reads them all

Options:
      --form <FORM>
          Read only the services a form declares

      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

  -f, --follow
          Keep reading as new lines arrive

      --force
          Take the stack from a run that claimed it and did not give it back

      --watch
          Read them on a screen that can be scrolled back and filtered

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --tail <TAIL>
          How many existing lines to begin with

          [default: 50]

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help
```
