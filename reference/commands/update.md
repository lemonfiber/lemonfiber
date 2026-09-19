# `lemonfiber update`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber update`

```text
Move something onto a newer version, naming which.

Two things here can be moved forward and the object is the whole of what tells them apart: `stack` moves the services somebody watches things on, and `self` moves this program. Neither is the smaller case of the other, so the object is required rather than defaulted — being handed the wrong one of these is being answered a question you did not ask.

Usage: lemonfiber update [OPTIONS] <COMMAND>

Commands:
  stack  Move the stack onto the image versions this build of lemonfiber pins
  self   Say where this copy of lemonfiber stands, and what moving it would come to
  help   Print this message or the help of the given subcommand(s)

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

## `lemonfiber update stack`

```text
Move the stack onto the image versions this build of lemonfiber pins.

A bare run changes nothing. It says which services would move, from which version to which, how large each step is, and which of them migrate state and so cannot be walked back. Run it again with `--confirm` to take the steps: a backup is taken first, and the services move one at a time with each proven to be answering before the next is touched.

Usage: lemonfiber update stack [OPTIONS]

Options:
      --json
          Print machine-readable output

      --service <SERVICE>
          Move one service instead of every one that has an update

      --confirm
          Go ahead and move them, having seen what each step costs

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --wait
          Let anything still downloading finish before the services are stopped

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber update self`

```text
Say where this copy of lemonfiber stands, and what moving it would come to.

Replaces nothing. It works out how this copy got onto the machine — Homebrew, Scoop, winget, cargo, the shell installer, or by hand — and prints the exact command for whichever tool owns it, because a binary that overwrote itself underneath a package manager leaves that manager holding a record of something that is no longer there.

The stack is untouched either way: containers run on their own, and this program only starts them. Nothing waits on the check, and a machine that cannot reach the release list is told so rather than stopped.

`--to` asks about one particular version instead of whatever is newest, which is how going back is asked for — along with whether that version reads the configuration already on this machine.

Usage: lemonfiber update self [OPTIONS]

Options:
      --json
          Print machine-readable output

      --to <VERSION>
          The version to move to, instead of whatever is newest

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
