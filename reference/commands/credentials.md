# `lemonfiber credentials`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber credentials`

```text
Say which credentials this stack holds, or act on one of them.

Every secret in the stack in one list, whoever produced it: what each is, what authenticates with it, where the value lives and where it stands — never the value itself — and what keeping them in files does and does not protect against. `--rotate` replaces one, proving the replacement before the existing value stops being in force; `--reveal` prints one, and asks first.

Usage: lemonfiber credentials [OPTIONS] [COMMAND]

Commands:
  revoke  End one grant: the credential is written out of the plugin's secrets file and its services reading it restart. Read first, then answered with the offer it names
  help    Print this message or the help of the given subcommand(s)

Options:
      --json
          Print machine-readable output

      --reveal <NAME>
          Print one credential's value. Asks for confirmation before it prints

      --dry-run
          Say what would happen, and change nothing

      --rotate <NAME>
          Replace one credential, proving the replacement against the live service before the existing value stops being the one in force

      --confirm
          Yes, print it in the clear — having read what that costs

      --force
          Take the stack from a run that claimed it and did not give it back

      --offer <OFFER>
          The offer a reading of a replacement named, which is the yes to it. Replacing the media server's internal certificate is read first and answered with its offer

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber credentials revoke`

```text
End one grant: the credential is written out of the plugin's secrets file and its services reading it restart. Read first, then answered with the offer it names

Usage: lemonfiber credentials revoke [OPTIONS] <PLUGIN> <CREDENTIAL>

Arguments:
  <PLUGIN>
          The plugin holding the grant

  <CREDENTIAL>
          The credential, by the setting `lemonfiber credentials` lists it under

Options:
      --json
          Print machine-readable output

      --offer <NAME>
          The offer the reading named, which is the yes to ending it

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
