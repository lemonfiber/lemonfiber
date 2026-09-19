# `lemonfiber down`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber down`

```text
Stop and remove what a form started

Usage: lemonfiber down [OPTIONS] [FORMS]...

Arguments:
  [FORMS]...
          The forms to stop; none stops everything the stack declares

Options:
      --json
          Print machine-readable output

      --service <NAME>
          Stop only these services, leaving the rest of the form running

      --dry-run
          Say what would happen, and change nothing

      --wait
          Let anything still downloading finish before stopping.

          Not for a stop of named services: what is in flight is a question about the download clients a form holds, so naming two services that are not download clients would wait on downloads stopping them cannot interrupt.

      --force
          Take the stack from a run that claimed it and did not give it back

      --yes
          Stop without asking about anything still downloading

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```
