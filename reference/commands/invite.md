# `lemonfiber invite`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber invite`

```text
Offer somebody in the house an account they can claim.

Makes them an account on the media server with no password on it, and prints the one address to send them. Whoever sets the first password claims it; an invitation nobody takes up is withdrawn.

Usage: lemonfiber invite [OPTIONS] <NAME>

Arguments:
  <NAME>
          What they will sign in as

Options:
      --json
          Print machine-readable output

      --library <NAME>
          Let them watch only these libraries, named as the media server names them; none lets them watch all of them

      --age-limit <AGE>
          Hold back anything the media server rates above this age — 0, 7, 12, 15 and 18 are the steps offered; none sets no limit at all

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --unrated <CHOICE>
          What to do about content the media server has no rating for; anybody being narrowed has it held back unless this says otherwise

          Possible values:
          - block: Hold it back
          - allow: Let it through

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```
