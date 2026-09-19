# `lemonfiber support`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber support`

```text
Gather everything a person helping you would ask for, with every value not named safe replaced by a stand-in.

A bare run writes nothing. It collects, redacts, and reads the result back looking for anything that still resembles a credential, then says what the bundle would hold and how large it is — so the decision to make a file worth attaching to a public thread is taken after seeing what goes in it. Run it again with `--write` to produce it.

Nothing is ever sent anywhere. The bundle is written here and stays here.

Usage: lemonfiber support [OPTIONS]

Options:
      --json
          Print machine-readable output

      --write
          Produce the bundle, having seen what it would hold

      --dry-run
          Say what would happen, and change nothing

      --out <PATH>
          Where to write it, instead of into this directory

      --force
          Take the stack from a run that claimed it and did not give it back

      --logs <LINES>
          How many log lines to take from each service

          [default: 200]

      --filenames
          Include media filenames, which are replaced by default

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --reveal <SETTING>
          Show one setting as it is, named exactly as the bundle names it.

          Repeatable, and refused without `--confirm` on the same run: a flag that publishes a credential is not one to honour because it turned up on a command line somebody copied.

      --confirm
          Confirm showing the settings named by `--reveal`

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```
