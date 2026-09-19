# `lemonfiber provenance`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber provenance`

```text
Say where each service comes from: its licence, its project, and the exact version this stack pins it at.

Everything bundled here is open source, and this is how you check that rather than take it on faith — the licence each service is published under, the project to go and read it at, and the image and tag actually being run.

It reads the stack description and nothing else, so it answers with the machine off and the containers down.

Usage: lemonfiber provenance [OPTIONS]

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
