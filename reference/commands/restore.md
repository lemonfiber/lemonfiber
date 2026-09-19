# `lemonfiber restore`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber restore`

```text
Restore your configuration from a backup archive.

Verifies the archive and lists what it holds before anything is overwritten. A restore onto a different data root is refused until `--repoint` accepts moving it to this machine's.

Name an archive, or name nothing and be told which backups this machine has kept.

Usage: lemonfiber restore [OPTIONS] [ARCHIVE]

Arguments:
  [ARCHIVE]
          The archive to restore from

Options:
      --json
          Print machine-readable output

      --repoint
          Accept re-pointing to this machine's data root where it differs

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
