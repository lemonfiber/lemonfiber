# `lemonfiber run`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber run`

```text
Run a run-and-exit service where one of its times fell due since its last run.

What this machine's scheduler calls every minute for each installed run-and-exit service. Where none of its times fell due it exits at once and records nothing; a run still going when a time falls due has that time skipped.

Usage: lemonfiber run [OPTIONS] <SERVICE>

Arguments:
  <SERVICE>
          The service

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
