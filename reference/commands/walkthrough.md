# `lemonfiber walkthrough`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber walkthrough`

```text
Add one thing, end to end, and watch every step of it happen.

The walk a first run is offered: search the indexers, grab a release, download it, import it, and see it appear in the library — narrated as it goes, so that afterwards you know what your stack does because you watched it do it. If any link is broken this is where it shows, with the step named and a way out.

Name something, or name nothing and be suggested something likely to work.

Usage: lemonfiber walkthrough [OPTIONS] [ITEM]...

Arguments:
  [ITEM]...
          What to add, named as you would say it

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
