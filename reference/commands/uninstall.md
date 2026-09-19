# `lemonfiber uninstall`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber uninstall`

```text
Take lemonfiber off this machine, at one of four removals.

`stop` removes nothing and stops the services. `services` removes the containers, the network they were on and the images pulled for them. `configuration` removes each service's own settings and everything lemonfiber keeps, credentials and all. `media` removes your library and your downloads, and is never bundled with any of the others.

Naming a removal lists exactly what it would take — every container, image and path, with what each occupies — and does nothing. `--confirm` carries it out. The one that reaches your library takes no bare yes: the listing prints a name for itself and `--agreed` answers that name, so an answer given against one reading of your disk cannot be spent on another.

Usage: lemonfiber uninstall [OPTIONS] <REMOVAL>

Arguments:
  <REMOVAL>
          Which removal: `stop`, `services`, `configuration` or `media`

          Possible values:
          - stop:          Remove nothing; stop the services
          - services:      Remove the containers, their network and the images pulled for them
          - configuration: Remove each service's own settings, everything lemonfiber keeps, and the credentials in both
          - media:         Remove the library and the downloads

Options:
      --confirm
          Go ahead, having read what would go

      --json
          Print machine-readable output

      --agreed <NAME>
          The listing being answered, as the run that printed it named it

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --wait
          Let anything still coming down finish before the services stop

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```
