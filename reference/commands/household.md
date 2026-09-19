# `lemonfiber household`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber household`

```text
Show who is in the household, what each may watch and ask for, and what each asked for.

Everybody the media server holds an account for — including those who have asked for nothing, and the invitations nobody has taken up yet. Each person carries what they may watch and when they were last seen; their requests read in the words they would use rather than the services' own, and each named one links to its full trace.

Each person also carries what they may ask for: how much a period allows them, how much of it is gone, and when there is room again. A request nobody has ruled on shows how long it has been waiting and about how much room it would want.

Name one of the four things underneath to change any of that, to answer one request that is waiting, or to arrange what becomes of the ones nobody answers.

Usage: lemonfiber household [OPTIONS] [COMMAND]

Commands:
  allow     Choose what happens to what the household asks for, and how much it may ask
  approve   Let one waiting request through, by the number the household list gives it
  decline   Turn one waiting request down, saying why
  expiring  Close the requests nobody has ruled on, once they have waited too long
  help      Print this message or the help of the given subcommand(s)

Options:
      --json
          Print machine-readable output

      --member <MEMBER>
          Narrow to one member, named the way you would say it

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

## `lemonfiber household allow`

```text
Choose what happens to what the household asks for, and how much it may ask.

Naming only a limit leaves the policy alone, and naming only a policy leaves the limit alone — saying nothing about something is not choosing it.

Television is counted a season at a time, because that is how the request service counts it: one ask for a six-season series spends six.

Usage: lemonfiber household allow [OPTIONS]

Options:
      --json
          Print machine-readable output

      --member <MEMBER>
          Set it for one person instead of for the whole household

      --dry-run
          Say what would happen, and change nothing

      --policy <POLICY>
          What happens to a request: trusted, within-a-limit, or everything-waits

      --force
          Take the stack from a run that claimed it and did not give it back

      --requests <REQUESTS>
          How many requests a period allows. Needs `--days` beside it

      --days <DAYS>
          How long that period is, in days. Needs `--requests` beside it

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber household approve`

```text
Let one waiting request through, by the number the household list gives it.

Refused where there is no room left on the disk, and said as the disk rather than as anybody's limit — raising a limit would change nothing.

Usage: lemonfiber household approve [OPTIONS] <REQUEST>

Arguments:
  <REQUEST>
          The request, by the number beside it

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

## `lemonfiber household decline`

```text
Turn one waiting request down, saying why.

The reason is required and it does not travel: the request service tells whoever asked that it was declined and carries no reason with it, so what you write here is yours to pass on.

Usage: lemonfiber household decline [OPTIONS] --reason <REASON> <REQUEST>

Arguments:
  <REQUEST>
          The request, by the number beside it

Options:
      --json
          Print machine-readable output

      --reason <REASON>
          Why, in a few words

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

## `lemonfiber household expiring`

```text
Close the requests nobody has ruled on, once they have waited too long.

Nothing is closed until you say how long is too long, and there is no period this chooses for you. Naming one records it and stops: the household is told about it, on the list you read and in the message each member is handed, before anything is closed.

Run with nothing named, it does the closing — and it holds this terminal until you stop it or arrange something else, because lemonfiber starts nothing by itself. Whoever asked is told why, at the address they already gave the request service.

Usage: lemonfiber household expiring [OPTIONS]

Options:
      --after <AFTER>
          How many days a request may wait before it is closed. Records it and stops

      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --never
          Stop closing anything for waiting, whatever was arranged before

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
