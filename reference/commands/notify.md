# `lemonfiber notify`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber notify`

```text
Add, prove, change, list or remove the back-ends the stack delivers its alerts to

Usage: lemonfiber notify [OPTIONS] <COMMAND>

Commands:
  list    List every back-end, where it reaches, whether it is proven and what became of its newest delivery
  add     Add a back-end, and prove it with `notify test` where its history can be read
  set     Change what a back-end is reached with, how it retries, or whether the household may receive on it. A changed secret holds it until `notify test` proves it again
  test    Send a back-end a test, and prove it where the test is seen arriving
  remove  Remove a back-end and forget what it is reached with
  help    Print this message or the help of the given subcommand(s)

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

## `lemonfiber notify list`

```text
List every back-end, where it reaches, whether it is proven and what became of its newest delivery

Usage: lemonfiber notify list [OPTIONS]

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

## `lemonfiber notify add`

```text
Add a back-end, and prove it with `notify test` where its history can be read

Usage: lemonfiber notify add [OPTIONS] --kind <KIND> --url <URL> <ID>

Arguments:
  <ID>
          What to call it, as in home-ntfy

Options:
      --json
          Print machine-readable output

      --kind <KIND>
          What it speaks

          Possible values:
          - ntfy:    An ntfy server's topic
          - gotify:  A gotify server's application
          - webhook: An address posted each message as JSON
          - apprise: An Apprise API address

      --dry-run
          Say what would happen, and change nothing

      --url <URL>
          The server's address, with no login in it

      --force
          Take the stack from a run that claimed it and did not give it back

      --topic <TOPIC>
          The ntfy topic it posts to

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --token-env <VARIABLE>
          The environment variable holding what it is posted with: an ntfy access token or `user:password`, a gotify application token, or a webhook's bearer token

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --reader-env <VARIABLE>
          The environment variable holding a gotify client token, which its history is read with to see a message arrive

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

      --hosted
          Accept that it is a hosted service, which what is delivered to it leaves for

      --attempts <COUNT>
          How many attempts a delivery is given, five where none is named

      --delays <MINUTES>
          The minutes waited before each attempt after the first, as in 1,5,15,60,360

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber notify set`

```text
Change what a back-end is reached with, how it retries, or whether the household may receive on it. A changed secret holds it until `notify test` proves it again

Usage: lemonfiber notify set [OPTIONS] <ID>

Arguments:
  <ID>
          The back-end, as the listing names it

Options:
      --json
          Print machine-readable output

      --token-env <VARIABLE>
          The environment variable holding what it is posted with: an ntfy access token or `user:password`, a gotify application token, or a webhook's bearer token

      --dry-run
          Say what would happen, and change nothing

      --reader-env <VARIABLE>
          The environment variable holding a gotify client token, which its history is read with to see a message arrive

      --attempts <COUNT>
          How many attempts a delivery is given, five where none is named

      --force
          Take the stack from a run that claimed it and did not give it back

      --delays <MINUTES>
          The minutes waited before each attempt after the first, as in 1,5,15,60,360

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --members <NAME>
          Offer it to the household under this name, which is all a member is shown

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

      --no-members
          Withdraw it from every member who chose it

  -h, --help
          Print help
```

## `lemonfiber notify test`

```text
Send a back-end a test, and prove it where the test is seen arriving

Usage: lemonfiber notify test [OPTIONS] <ID>

Arguments:
  <ID>
          The back-end, as the listing names it

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

## `lemonfiber notify remove`

```text
Remove a back-end and forget what it is reached with

Usage: lemonfiber notify remove [OPTIONS] <ID>

Arguments:
  <ID>
          The back-end, as the listing names it

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
