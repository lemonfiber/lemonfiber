# `lemonfiber hosting`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber hosting`

```text
Say what this machine keeps running when no terminal is open.

Two of this program's commands have to keep running to be worth anything — the guard on the data location, and the clock that closes requests nobody rules on — and both stop when the window they were started in closes. This is what hands them to the machine instead.

Asked nothing it reports what stands between each of them and this machine: whether one is installed, whether the system is actually running it, where its words are written, and — on a platform this program cannot configure — what to do instead of it. Installed is not running, and the two are never reported as one thing.

Name one of the two words underneath to install one or take it back.

Usage: lemonfiber hosting [OPTIONS] [COMMAND]

Commands:
  install  Have this machine keep one of them running, now and after a restart
  remove   Take one back off this machine, leaving nothing behind
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
          Print help (see a summary with '-h')
```

## `lemonfiber hosting install`

```text
Have this machine keep one of them running, now and after a restart.

Installs it into your own account — no administrator rights, and nothing another account on this machine inherits. It starts it as well as installing it, and says so, along with where a command with no terminal writes what it would have said in one.

Usage: lemonfiber hosting install [OPTIONS] <WHAT> [FORMS]...

Arguments:
  <WHAT>
          Which one: the guard on the data location, the clock on requests, or the start that brings the stack back after a restart

          Possible values:
          - watch:    The guard on the data location
          - expiring: The clock that closes requests nobody has ruled on
          - boot:     The start that brings the stack back after this machine restarts

  [FORMS]...
          The forms the guard stops if the data location is lost. The guard alone takes them, and it will not be installed without them

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

## `lemonfiber hosting remove`

```text
Take one back off this machine, leaving nothing behind.

The service definition, its registration, and its place in your login items. Asked about one that is not installed, it says so rather than failing.

Usage: lemonfiber hosting remove [OPTIONS] <WHAT>

Arguments:
  <WHAT>
          Which one

          Possible values:
          - watch:    The guard on the data location
          - expiring: The clock that closes requests nobody has ruled on
          - boot:     The start that brings the stack back after this machine restarts

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
