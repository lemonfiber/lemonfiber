# `lemonfiber catalogue`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber catalogue`

```text
Say what each service in this stack is for, and what became of any it dropped.

Nineteen names convey nothing on their own. This gives each of them a sentence in plain language — what it does for you, what you lose while it is down, and how much that loss matters — so a stack you can list becomes a stack you can judge. Anything the stack used to carry and no longer does is listed after them, with why it went and what took its place.

It reads the stack description and nothing else, so it answers with the machine off and the containers down.

Usage: lemonfiber catalogue [OPTIONS]

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
