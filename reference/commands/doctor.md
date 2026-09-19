# `lemonfiber doctor`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber doctor`

```text
Run the checks that prove the stack is doing what it should

Usage: lemonfiber doctor [OPTIONS]

Options:
      --json
          Print machine-readable output

      --only <CATEGORY_OR_CHECK>
          Run one category of check, such as `vpn`, or one check by the name a finding gives it, such as `vpn.killswitch`

      --disruptive
          Include the checks that disturb the running system

      --dry-run
          Say what would happen, and change nothing

      --accept <CHECK>
          Answer a warning about a choice — `vpn.unprotected`, say — so it stops leading. Only something this run warns about can be answered

      --force
          Take the stack from a run that claimed it and did not give it back

      --fix
          Offer to put right what lemonfiber can, asking about each first.

          A plain run only looks. This one says what each repair would do and what else changes if it does, and waits to be told.

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --yes
          Carry the repairs out without asking, having decided in advance

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

      --fix-disruptive
          Include the checks that disturb the running system while repairing.

          Named apart from the field it sits beside: `doctor` already has a `--disruptive`, and clap keys an argument by the field name unless told otherwise — so two flags that read differently on the command line would be one argument underneath.

      --undo
          Put back what the last repair changed.

          Asked for the same way a repair is, because it is the same errand read backwards. It reverses that one repair and nothing else: the wiring lemonfiber seeded and the choices your first run wrote are left where they are.

  -h, --help
          Print help (see a summary with '-h')
```
