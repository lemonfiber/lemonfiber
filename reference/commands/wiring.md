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

Named on its own it says what the change would come to — what fills the capability now, what asks for it, and what it would leave with nothing filling it — changes nothing, and prints a name for that offer; answering with that name is the yes. The wiring is read again first, and an answer given for a different reading is refused, naming what moved.

Usage: lemonfiber wiring fill [OPTIONS] <CAPABILITY> <SERVICE>

Arguments:
  <CAPABILITY>
          The capability whose filler changes, such as `identity.source`

  <SERVICE>
          The service to fill it

Options:
      --json
          Print machine-readable output

      --reason <TEXT>
          Why you chose it, recorded with the choice and read back beside it

      --dry-run
          Say what would happen, and change nothing

      --offer <NAME>
          The offer being answered, as the run that made it printed it

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
