# `lemonfiber quality`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber quality`

```text
Choose how good your media should look, in plain language

Usage: lemonfiber quality [OPTIONS] <COMMAND>

Commands:
  show     Show the quality choice in force, and what each preset means and costs
  set      Choose a preset — for everything, or for one media type
  reapply  Re-assert the recorded preset over a Recyclarr config you have hand-edited
  upgrade  Upgrade existing content to the chosen quality — re-download what is already here at the higher quality
  help     Print this message or the help of the given subcommand(s)

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
          Print help
```

## `lemonfiber quality show`

```text
Show the quality choice in force, and what each preset means and costs

Usage: lemonfiber quality show [OPTIONS]

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
          Print help
```

## `lemonfiber quality set`

```text
Choose a preset — for everything, or for one media type

Usage: lemonfiber quality set [OPTIONS] <PRESET>

Arguments:
  <PRESET>
          The preset: space-saving, balanced, high-quality, or maximum

Options:
      --for <MEDIA_TYPE>
          Apply it to one media type (tv or movies) rather than everything

      --json
          Print machine-readable output

      --confirm
          Confirm a choice this machine would have to transcode in software

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
          Print help
```

## `lemonfiber quality reapply`

```text
Re-assert the recorded preset over a Recyclarr config you have hand-edited.

An ordinary run keeps your edits; this is the explicit consent to let the preset win instead.

Usage: lemonfiber quality reapply [OPTIONS]

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

## `lemonfiber quality upgrade`

```text
Upgrade existing content to the chosen quality — re-download what is already here at the higher quality.

A large, bandwidth-expensive operation, separate from a preset change (which only affects future acquisitions). States the cost and does nothing until `--confirm`.

Usage: lemonfiber quality upgrade [OPTIONS]

Options:
      --confirm
          Go ahead and trigger the re-search, having seen the cost

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
