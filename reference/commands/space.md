# `lemonfiber space`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber space`

```text
Account for the disk: where the room went, when it runs out, what can go.

Says when the disk will be full rather than that it already is, counting what is queued against what is left. Breaks the rest down by where it went, and marks what could be got back — the downloads nothing ever imported and the archives already unpacked cost nothing at all. Those two are what `--confirm` takes, and nothing else ever is: a torrent still seeding is named with what removing it does to your standing with the tracker and left with you, and nothing you asked to be left alone is on offer at any level of fullness.

Usage: lemonfiber space [OPTIONS]

Options:
      --confirm
          Go ahead and remove what was listed as costing nothing

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
