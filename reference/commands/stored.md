# `lemonfiber stored`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber stored`

```text
List what lemonfiber keeps on this machine, where it is, and why.

Everything it writes sits under two directories. This names each thing under them, says what it is for, and marks the ones holding a credential — and it names what is *not* lemonfiber's, because your library being absent from the list is the part worth being sure about.

Usage: lemonfiber stored [OPTIONS]

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
