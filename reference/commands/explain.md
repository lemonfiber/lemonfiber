# `lemonfiber explain`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber explain`

```text
Say what one of this product's words means.

A report explains the words it used underneath itself, in a sentence. This is the longer form, for somebody who wants it — nothing needs it in order to act, which is the difference between an explanation offered and one imposed.

Name a word, or name nothing and be told which words there are.

Usage: lemonfiber explain [OPTIONS] [WORD]...

Arguments:
  [WORD]...
          The word, as you would say it

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
