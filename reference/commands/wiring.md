# `lemonfiber wiring`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber wiring`

```text
Say what this stack wires to what, and how each link was settled.

A link asks for a capability — an identity source, a torrent client — and whatever provides it is what the link reaches, so putting something else in its place is one setting rather than a hunt for everything that named it. One kept to a named service is shown as one, with its reason, and where two claim the same thing `fill` is how you choose. Non-zero where nothing provides it.

Usage: lemonfiber wiring [OPTIONS] [COMMAND]

Commands:
  fill  Choose which service fills a capability, so everything that asked reaches it
  help  Print this message or the help of the given subcommand(s)

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

## `lemonfiber wiring fill`

```text
Choose which service fills a capability, so everything that asked reaches it.

Substituting is this and nothing else. A link asks for a capability, and which service answers is a setting — so it is recorded, it shows in the history, and `undo` puts it back.

What the change would leave with nothing filling it is said before it is made. Run it with `--dry-run` to see that and write nothing.

Usage: lemonfiber wiring fill [OPTIONS] <CAPABILITY> <SERVICE>

Arguments:
  <CAPABILITY>
          The capability whose filler changes, such as `identity.source`

  <SERVICE>
          The service to fill it

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
