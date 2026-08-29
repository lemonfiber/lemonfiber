# `lemonfiber` — command reference

Generated from the command line's own declarations. Run `just reference` to rewrite it.

## `lemonfiber`

```text
Orchestrates a fully open-source, self-hosted media stack

Usage: lemonfiber [OPTIONS] [COMMAND]

Commands:
  setup        Set up the stack by answering a few questions
  version      Report the versions in play
  forms        List the forms this stack has, and what each one is for
  up           Start a form, or the union of several
  down         Stop and remove what a form started
  switch       Make these forms the active set, leaving shared services running
  restart      Restart services without touching the rest
  pull         Fetch newer images without applying them
  ps           Report what each service is actually doing
  logs         Show what services are saying
  config       Read or change one setting
  quality      Choose how good your media should look, in plain language
  doctor       Run the checks that prove the stack is doing what it should
  watch        Guard the data location while forms run, stopping them if it disappears
  trace        Follow one show or film across the services — "where is my show?"
  household    Show what the household asked for, and where each request stands
  walkthrough  Add one thing, end to end, and watch every step of it happen
  explain      Say what one of this product's words means
  stuck        List the items whose downloads are stuck — the landing point for "N stuck", each named so `lemonfiber trace` follows it on its own
  front-door   Name the one address to send somebody who lives here
  outbound     List everything that leaves this machine, and what refusing each of it costs
  stored       List what lemonfiber keeps on this machine, where it is, and why
  clients      Say which app to watch on, for each kind of device somebody in the house has
  invite       Offer somebody in the house an account they can claim
  forget       Remove everything lemonfiber keeps on this machine
  seed         Wire the stack's services to each other, idempotently
  adopt        Adopt your current edits as lemonfiber's expected state
  reset        Put the stack back to lemonfiber's own state, reverting every edit you made
  backup       Back up your configuration to an archive, so it stops being precious
  support      Gather everything a person helping you would ask for, with every value not named safe replaced by a stand-in
  ui           Serve the web interface, for as long as you leave it running
  restore      Restore your configuration from a backup archive
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

  -h, --help
          Print help

  -V, --version
          Print version
```

## `lemonfiber setup`

```text
Set up the stack by answering a few questions.

Interactive by default. Given the flags below, it runs unattended: each answers a question the wizard would otherwise ask, and `--yes` stands in for the confirmation. A non-interactive run missing a flag it needs is told which, rather than left waiting on input that will not come.

Usage: lemonfiber setup [OPTIONS]

Options:
      --json
          Print machine-readable output

      --status
          Report where setup stands and ask nothing. Takes precedence over the answers below, so a run that asks where it is never also answers

      --dry-run
          Say what would happen, and change nothing

      --yes
          Apply without a prompt to confirm — required for an unattended run

      --force
          Take the stack from a run that claimed it and did not give it back

      --protocols <PROTOCOLS>
          How to fetch content: `both`, `usenet`, `torrent`, or `none`

      --data-location <PATH>
          Where the library and downloads live

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --indexer-url <URL>
          An indexer's API base URL

      --indexer-key <KEY>
          The indexer's API key

      --usenet-host <HOST>
          The Usenet provider's hostname

      --usenet-port <PORT>
          The port the Usenet provider answers on (defaults to 563)

      --usenet-user <USER>
          The Usenet account username

      --usenet-pass <PASS>
          The Usenet account password

      --usenet-tls <BOOL>
          Whether the Usenet connection uses TLS (defaults to yes)

          [possible values: true, false]

      --library <MODE>
          How to serve the library: `docker`, `native`, or `none`

      --service-user <UID:GID>
          The container user, as `UID:GID`

      --vpn <BOOL>
          Whether a VPN carries the torrent traffic. Where torrents are chosen and this is absent, the run proceeds unprotected and records that it did

          [possible values: true, false]

      --household <BOOL>
          Whether others in the home will use it

          [possible values: true, false]

      --notifications <APPETITE>
          What to be told about: `problems`, `completions`, or `everything`

      --autostart <BOOL>
          Whether to start the stack when the machine boots

          [possible values: true, false]

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber version`

```text
Report the versions in play

Usage: lemonfiber version [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber forms`

```text
List the forms this stack has, and what each one is for.

A form says which part of the stack to run. They come from the stack rather than from lemonfiber, so a stack of your own names its own.

Naming one says what starting it would come to — the services it holds, and anything your configuration leaves out — without starting anything.

Usage: lemonfiber forms [OPTIONS] [FORMS]...

Arguments:
  [FORMS]...
          The forms to describe; none lists them all

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber up`

```text
Start a form, or the union of several

Usage: lemonfiber up [OPTIONS] [FORMS]...

Arguments:
  [FORMS]...
          The forms to start; none starts everything the stack declares

Options:
      --json
          Print machine-readable output

      --service <NAME>
          Start only these services, leaving the rest of the form alone

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber down`

```text
Stop and remove what a form started

Usage: lemonfiber down [OPTIONS] [FORMS]...

Arguments:
  [FORMS]...
          The forms to stop; none stops everything the stack declares

Options:
      --json
          Print machine-readable output

      --service <NAME>
          Stop only these services, leaving the rest of the form running

      --dry-run
          Say what would happen, and change nothing

      --wait
          Let anything still downloading finish before stopping.

          Not for a stop of named services: what is in flight is a question about the download clients a form holds, so naming two services that are not download clients would wait on downloads stopping them cannot interrupt.

      --force
          Take the stack from a run that claimed it and did not give it back

      --yes
          Stop without asking about anything still downloading

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber switch`

```text
Make these forms the active set, leaving shared services running.

Only what falls outside the new shape is stopped. A service the old shape and the new one both hold keeps running rather than being restarted, so a download in flight is not interrupted to change the stack around it.

Usage: lemonfiber switch [OPTIONS] <FORMS>...

Arguments:
  <FORMS>...
          The forms to switch to

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber restart`

```text
Restart services without touching the rest

Usage: lemonfiber restart [OPTIONS] <FORM> [SERVICES]...

Arguments:
  <FORM>
          The form holding them

  [SERVICES]...
          The services to restart; none restarts the whole form

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber pull`

```text
Fetch newer images without applying them

Usage: lemonfiber pull [OPTIONS] <FORMS>...

Arguments:
  <FORMS>...
          The forms whose images to fetch

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber ps`

```text
Report what each service is actually doing

Usage: lemonfiber ps [OPTIONS] [FORMS]...

Arguments:
  [FORMS]...
          The forms to report on; none reports the whole stack

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber logs`

```text
Show what services are saying

Usage: lemonfiber logs [OPTIONS] [SERVICES]...

Arguments:
  [SERVICES]...
          The services to read; none reads them all

Options:
      --form <FORM>
          Read only the services a form declares

      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

  -f, --follow
          Keep reading as new lines arrive

      --force
          Take the stack from a run that claimed it and did not give it back

      --watch
          Read them on a screen that can be scrolled back and filtered

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --tail <TAIL>
          How many existing lines to begin with

          [default: 50]

  -h, --help
          Print help
```

## `lemonfiber config`

```text
Read or change one setting

Usage: lemonfiber config [OPTIONS] <COMMAND>

Commands:
  get   Read one setting
  set   Change one setting
  show  Show every setting, with credentials withheld
  help  Print this message or the help of the given subcommand(s)

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber config get`

```text
Read one setting

Usage: lemonfiber config get [OPTIONS] <KEY>

Arguments:
  <KEY>
          The setting to read

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber config set`

```text
Change one setting

Usage: lemonfiber config set [OPTIONS] <KEY> <VALUE>

Arguments:
  <KEY>
          The setting to change

  <VALUE>
          What to change it to

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber config show`

```text
Show every setting, with credentials withheld

Usage: lemonfiber config show [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber quality`

```text
Choose how good your media should look, in plain language

Usage: lemonfiber quality [OPTIONS] <COMMAND>

Commands:
  show     Show the quality choice in force, and what each preset means and costs
  set      Choose a preset — for everything, or for one media type
  reapply  Re-assert the recorded preset over a Recyclarr config you have hand-edited
  upgrade  Upgrade existing content to the chosen quality — re-download what is already here at the higher quality
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

  -h, --help
          Print help
```

## `lemonfiber quality show`

```text
Show the quality choice in force, and what each preset means and costs

Usage: lemonfiber quality show [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber quality set`

```text
Choose a preset — for everything, or for one media type

Usage: lemonfiber quality set [OPTIONS] <PRESET>

Arguments:
  <PRESET>
          The preset: space-saving, balanced, high-quality, or maximum

Options:
      --for <MEDIA_TYPE>
          Apply it to one media type (tv or movies) rather than everything

      --json
          Print machine-readable output

      --confirm
          Confirm a choice this machine would have to transcode in software

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber quality reapply`

```text
Re-assert the recorded preset over a Recyclarr config you have hand-edited.

An ordinary run keeps your edits; this is the explicit consent to let the preset win instead.

Usage: lemonfiber quality reapply [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber quality upgrade`

```text
Upgrade existing content to the chosen quality — re-download what is already here at the higher quality.

A large, bandwidth-expensive operation, separate from a preset change (which only affects future acquisitions). States the cost and does nothing until `--confirm`.

Usage: lemonfiber quality upgrade [OPTIONS]

Options:
      --confirm
          Go ahead and trigger the re-search, having seen the cost

      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

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

      --yes
          Carry the repairs out without asking, having decided in advance

      --fix-disruptive
          Include the checks that disturb the running system while repairing.

          Named apart from the field it sits beside: `doctor` already has a `--disruptive`, and clap keys an argument by the field name unless told otherwise — so two flags that read differently on the command line would be one argument underneath.

      --undo
          Put back what the last repair changed.

          Asked for the same way a repair is, because it is the same errand read backwards. It reverses that one repair and nothing else: the wiring lemonfiber seeded and the choices your first run wrote are left where they are.

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber watch`

```text
Guard the data location while forms run, stopping them if it disappears

Usage: lemonfiber watch [OPTIONS] <FORMS>...

Arguments:
  <FORMS>...
          The forms to stop if the data location is lost

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber trace`

```text
Follow one show or film across the services — "where is my show?".

Searched for the way you would name it, not by an internal id. Reports how far it got and, where it plainly stopped, why. A show is reported season by season: how many episodes are here, and what each one that is not is waiting on.

Usage: lemonfiber trace [OPTIONS] <TERM>...

Arguments:
  <TERM>...
          The show or film to follow, named as you would say it

Options:
      --json
          Print machine-readable output

      --season <SEASON>
          Narrow to one season, instead of every season of the show

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber household`

```text
Show what the household asked for, and where each request stands.

Grouped by whoever asked, in the words they would use rather than the services' own. Each named request links to its full trace.

Usage: lemonfiber household [OPTIONS]

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

  -h, --help
          Print help (see a summary with '-h')
```

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

  -h, --help
          Print help (see a summary with '-h')
```

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

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber stuck`

```text
List the items whose downloads are stuck — the landing point for "N stuck", each named so `lemonfiber trace` follows it on its own

Usage: lemonfiber stuck [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber front-door`

```text
Name the one address to send somebody who lives here.

The stack publishes several things to your network and only one of them is somewhere to begin. This says which, why the others are not, and — where this stack runs nothing anybody could begin at — that there is no address to send rather than naming the nearest thing that would open.

Usage: lemonfiber front-door [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber outbound`

```text
List everything that leaves this machine, and what refusing each of it costs.

lemonfiber's own requests first — where each goes, why, exactly what travels, whether it is on, the setting that switches it off and what stops working when it is — then the requests the stack's own services make, which are theirs rather than lemonfiber's.

Usage: lemonfiber outbound [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber stored`

```text
List what lemonfiber keeps on this machine, where it is, and why.

Everything it writes sits under two directories. This names each thing under them, says what it is for, and marks the ones holding a credential — and it names what is *not* lemonfiber's, because your library being absent from the list is the part worth being sure about.

Usage: lemonfiber stored [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber clients`

```text
Say which app to watch on, for each kind of device somebody in the house has.

The client landscape is uneven and it matters which app is used: some devices have an official one that works, and a smart television may have nothing worth using. This says which is which, names a browser as the answer that always works and needs no installation, and where a device is badly served says what to do instead rather than leaving somebody to find out by failing.

Usage: lemonfiber clients [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

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

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber forget`

```text
Remove everything lemonfiber keeps on this machine.

The two directories and everything under them. Your library, your downloads and the containers are not lemonfiber's and are never touched. Because it throws work away it lists what would go and does nothing until `--confirm`.

Usage: lemonfiber forget [OPTIONS]

Options:
      --confirm
          Go ahead and remove it, having seen what would go

      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber seed`

```text
Wire the stack's services to each other, idempotently

Usage: lemonfiber seed [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber adopt`

```text
Adopt your current edits as lemonfiber's expected state.

A value you changed by hand reports as drift until you adopt it; once adopted it is kept across future seeds and restores. Wires what is missing as a seed does, and promotes every drifted value to yours.

Usage: lemonfiber adopt [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber reset`

```text
Put the stack back to lemonfiber's own state, reverting every edit you made.

The opposite of adopt: it discards your hand-edits to the stack files and restores lemonfiber's own. Because it throws work away, it names exactly what will be lost and does nothing until `--confirm` — run it once to see the diffs, again with `--confirm` to reset.

Usage: lemonfiber reset [OPTIONS]

Options:
      --confirm
          Go ahead and revert, having seen what will be lost

      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber backup`

```text
Back up your configuration to an archive, so it stops being precious

Usage: lemonfiber backup [OPTIONS]

Options:
      --json
          Print machine-readable output

      --service <SERVICE>
          Back up one service's configuration instead of the whole stack

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

  -h, --help
          Print help
```

## `lemonfiber support`

```text
Gather everything a person helping you would ask for, with every value not named safe replaced by a stand-in.

A bare run writes nothing. It collects, redacts, and reads the result back looking for anything that still resembles a credential, then says what the bundle would hold and how large it is — so the decision to make a file worth attaching to a public thread is taken after seeing what goes in it. Run it again with `--write` to produce it.

Nothing is ever sent anywhere. The bundle is written here and stays here.

Usage: lemonfiber support [OPTIONS]

Options:
      --json
          Print machine-readable output

      --write
          Produce the bundle, having seen what it would hold

      --dry-run
          Say what would happen, and change nothing

      --out <PATH>
          Where to write it, instead of into this directory

      --force
          Take the stack from a run that claimed it and did not give it back

      --logs <LINES>
          How many log lines to take from each service

          [default: 200]

      --filenames
          Include media filenames, which are replaced by default

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --reveal <SETTING>
          Show one setting as it is, named exactly as the bundle names it.

          Repeatable, and refused without `--confirm` on the same run: a flag that publishes a credential is not one to honour because it turned up on a command line somebody copied.

      --confirm
          Confirm showing the settings named by `--reveal`

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber ui`

```text
Serve the web interface, for as long as you leave it running.

Started when you ask for it and not before: nothing is installed, nothing keeps running afterwards, and stopping it leaves nothing behind. It listens on this machine only.

The connection is not encrypted, which it says when it starts, along with the whole address it was given and the token every request to it must carry. The token is minted for this run, printed once here, and kept nowhere else.

Usage: lemonfiber ui [OPTIONS]

Options:
      --json
          Print machine-readable output

      --port <PORT>
          The port to listen on. Without it, whichever one is free is used and the whole address is printed

      --dry-run
          Say what would happen, and change nothing

      --no-browser
          Do not ask this desktop to open a browser

      --assets <PATH>
          Serve the interface from this directory rather than from the binary.

          No build carries a web app of its own yet, so this is the only way to serve one. A build asked without it says as much rather than answering with an empty page.

      --force
          Take the stack from a run that claimed it and did not give it back

      --lan
          Offer this to your network, rather than to this machine only.

          Refused unless a password has been set. This surface can start, stop and reconfigure everything and reaches every password the system holds, so it is not offered to a network with nothing in front of it.

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --set-password
          Set the password this surface asks for, before it starts.

          Asked for at the keyboard and never on this line: a password typed as an argument is a password in your shell's history and in the list of processes this machine is running.

  -h, --help
          Print help (see a summary with '-h')
```

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

  -h, --help
          Print help (see a summary with '-h')
```
