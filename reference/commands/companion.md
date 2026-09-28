# `lemonfiber companion`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber companion`

```text
Pair a phone with this stack, or replace the certificate a paired phone pins

Usage: lemonfiber companion [OPTIONS] <COMMAND>

Commands:
  pair         Make pairing material for a phone: a code for its camera and the same text to type
  certificate  Replace the certificate the web interface presents to a paired phone
  help         Print this message or the help of the given subcommand(s)

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

## `lemonfiber companion pair`

```text
Make pairing material for a phone: a code for its camera and the same text to type.

It names the address the phone reaches, the certificate that address presents, when the material stops being good, and this stack's own identifier. It carries no password, and scanning it lets nobody in: the phone still signs in with yours. Needs the web interface served encrypted on your network, on a port that stays the same — `lemonfiber ui --lan --tls --port <port>`.

Usage: lemonfiber companion pair [OPTIONS]

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

## `lemonfiber companion certificate`

```text
Replace the certificate the web interface presents to a paired phone.

Every phone paired with this machine refuses it afterwards, as it should for a certificate it was never shown, until it is paired again. So on its own this says that and replaces nothing; `--confirm` replaces it.

Usage: lemonfiber companion certificate [OPTIONS]

Options:
      --confirm
          Replace it, having been told what replacing it costs

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
