# Surface parity

Which of the command line's requests each other surface can reach, as the code
stands.

The rule — every action available from every surface, and the four exceptions to
it — is the spec's
[G1](https://github.com/lemonfiber/spec/blob/main/10-functional/features/g-ux/g1-interface-tiers.md).
That page says what will never be built anywhere and why. This one says what is
built here and what is missing, which is a different question and changes far more
often.

The two are kept apart deliberately. An exception is permanent and belongs where
the requirement is; a gap is temporary and belongs where the code is. Writing both
in one place is how a gap comes to be read as an exception, which is the failure
`G1-R1` exists to prevent — and the difference between the two claims is the whole
of what that requirement asks for.

## Where each surface's offer is declared

| Surface | Declared in | Read as |
|---------|-------------|---------|
| Command line | [`cli.rs`](../../crates/lemonfiber/src/cli.rs) | the `Request` enum, which clap renders |
| Web, writes | [`named.rs`](../../crates/lemonfiber-api/src/actions/named.rs) | `OFFERED`, one name per action |
| Web, reads | [`read.rs`](../../crates/lemonfiber-api/src/read.rs) · [`setup.rs`](../../crates/lemonfiber-api/src/setup.rs) · [`jobs.rs`](../../crates/lemonfiber-api/src/jobs.rs) | the routes each declares |
| Terminal, writes | [`acting/offer.rs`](../../crates/lemonfiber/src/acting/offer.rs) | `OFFERED`, one key per action |
| Terminal, reads | [`acting/question.rs`](../../crates/lemonfiber/src/acting/question.rs) · [`terminal.rs`](../../crates/lemonfiber/src/terminal.rs) | the questions the list holds, and the screens the loop opens |

The table below is checked against the first three by
[`surface_parity.rs`](../../crates/lemonfiber/tests/surface_parity.rs): a request
with no row fails, a row naming an action or a route that does not exist fails,
and an action or a route the web offers that no row accounts for fails. A route
that answers no command-line request at all — the stream, the path actions are
asked for through, and the one a job's name is redeemed at — is declared there
by name, so adding a route is a decision somebody makes rather than one that
happens.

The terminal column is still the one a reader has to check by eye, but for a
smaller reason than before. Its actions and its questions are each declared in a
list now rather than being knowable only by reading a match arm at a time, and a
test beside each list holds every entry to something the web already offers — so an
action or a read this screen alone could reach fails there. What nothing joins is
either list to this column: `acting/` lives in the binary rather than in the library
this test reads, so a row claiming `dashboard` is a claim about a file rather than a
claim read out of one.

## The table

**Web** names the actions and routes that reach a request, or `none`, or
`intrinsic`. **Terminal** names the screen that offers it, or `none`, or
`intrinsic`. Either may add `partial`, which means some of the request is reachable
and the rest is named in **Standing**.

| Request | Web | Terminal | Standing |
|---------|-----|----------|----------|
| `setup` | `/api/setup`, `/api/setup/answer`, `/api/setup/next`, `/api/setup/back`, `/api/setup/apply`, partial | wizard | Completable from a browser and from a terminal. Two affordances are terminal-only — proving a credential against the provider while the answer is being given, and choosing how to recover an interrupted apply — and neither blocks finishing setup. |
| `version` | `/api/version` | dashboard | Reachable from all three, and the cheapest read on this page: no arguments, and an answer the core already renders. The dashboard asks for it off the list one key opens, and a request with no arguments has nothing left over to leave out. |
| `forms` | `/api/forms` | dashboard, partial | Served on the web, and through one endpoint because the command line spells it as one request: naming no form lists what the stack declares, naming some says what starting those would come to. The profile carried on `/api/services` is a Compose profile and not a form, and neither list contains the other, so this needed an endpoint of its own rather than a reading of that one. The dashboard asks the listing half. Naming a form to see what starting it would come to is not on the list, and it is the half the screen loses least by: the five actions each open the same forms and say what they are about before they run. |
| `up` | `up`, partial | dashboard, partial | Starting a form is offered on both, and on the terminal the whole stack is a choice too, because the command carries an empty list. Starting only some of a form's services is not reachable at all: `--service` never reaches a `Command` — the command line runs its own streamed start around it — so there is nothing to hand them to, and services named to this action are refused rather than dropped for a start of the whole form. Whether that is its own request, the way `Halt` is not `Down`, is a question for the core. Naming several forms at once is reachable from a browser, which sends the list whole, and not from the dashboard, whose list takes one. |
| `down` | `down`, partial | dashboard, partial | The teardown is offered on both, and so is stopping named services on the web, which is `Command::Halt` rather than an argument to this one — the terminal offers neither that nor several forms at once. Letting anything still downloading finish first is on no other surface: `--wait` is a loop the command line runs around a queue reading before it asks for the stop, not something `Command::Down` carries. Its companion `--yes` answers a prompt no machine-readable run is put; the terminal asks its own question instead, on a screen where one key reaches a teardown. |
| `switch` | `switch` | dashboard, partial | Reachable in full from a browser. It refuses an empty `forms`, and `/api/forms` serves the names — which is what it was waiting on; the dashboard asks the same list of the core and offers the names it comes back with, so a stack declaring none is refused there in the same words. What the terminal leaves is naming several forms at once, its list taking one. |
| `restart` | `restart` | dashboard, partial | As `switch` on both counts. The terminal leaves the same two things: several forms at once, and the named services `--service` restarts, which its list has no way to name. |
| `pull` | `pull` | dashboard, partial | As `switch`: it refuses an empty `forms`, which `/api/forms` and the dashboard's own list both serve. The terminal leaves naming several forms at once. It is the one of the five that stops nothing, and it is asked about before it runs anyway, because it can spend an hour of somebody's connection. |
| `ps` | `/api/status`, `/api/services` | dashboard | Reachable from all three. The dashboard and the endpoints are fed by the same gather. |
| `logs` | `/api/logs`, partial | viewer | The scrollback is a read and is served. Following is not: the event stream carries the dashboard gather and the narration a wait produces, never a service's own lines, so a browser cannot watch them arrive. `--watch` is the terminal's own rendering of the same lines, not a separate request. |
| `config` | `config-set`, `/api/config` | dashboard, partial | Reachable in full from a browser. Writing arrived before reading, so a browser could set a value it had no way to read back; `/api/config` answers both halves of the read the way the command line spells them — naming no setting shows every one, naming one reads that one. Credentials are withheld where the settings are read rather than where they are printed, so both the endpoint and the dashboard show what `config show` prints, and a guard refuses a screen that reads the file for itself. The terminal asks for the whole listing and not for one setting by name: the answer is a box that moves through the settings, so reading one of them is scrolling to it rather than typing it exactly. Changing one is offered nowhere but the browser, like every other write. |
| `quality` | `quality-set`, `quality-reapply`, `quality-upgrade`, `/api/quality` | dashboard, partial | Reachable in full from a browser. `/api/quality` serves the read the three writes were being made without: the preset in force, what each one means, and what it costs, which is the screen a browser is best at. The dashboard asks the same read and offers none of the three writes. |
| `doctor` | `/api/checks`, `/api/storage`, partial | dashboard, partial | The diagnosis is served. Repair is not: `--fix`, `--yes`, `--fix-disruptive`, `--undo` and `--accept` have no web form, and an offer-and-consent flow is something HTML does better than a terminal prompt. The dashboard shows storage and VPN facts the diagnosis also reads, without being the diagnosis. |
| `watch` | none | none | Unbuilt. It is long-running work that ends in one report, which is exactly the shape the web already answers with a job name — and a guard started from a browser and left running is the useful case, not the awkward one. What it needs that a command does not is a way to stop it, since there is no terminal to interrupt. |
| `trace` | `/api/trace` | dashboard, partial | Served on the web, and it completes a screen that half-existed: `/api/requests` reports what the household asked for, and this follows one of them. What to follow is one query parameter — the command line takes it as words so it can be typed unquoted, and a query string carries the title whole. It is the one question the dashboard has to be given something before it can ask, so taking it opens a line to type the title on, and an empty one is refused in the sentence a browser is refused with. Narrowing to one season is not offered there: it is a second thing to type for an answer that already reads season by season. |
| `household` | `/api/requests` | dashboard, partial | Served on the web, and asked off the dashboard's own list of questions. Narrowing to one member is not offered there: the answer is grouped by whoever asked, so a household of four is four headings rather than a list to search, and naming one would be a second thing to type for a smaller version of what is already on the screen. |
| `walkthrough` | none | none | Unbuilt, and the surface it is least built for is the one it was designed for: it narrates for minutes, which is a job plus the event stream, and its audience is a first-time operator who is likelier to be in a browser than in a shell. |
| `explain` | `/api/explain` | glossary | Reachable in full, and the one read that needs neither a stack nor a daemon: the words are a table compiled into the binary, so a browser meeting one in a failure can ask what it means while the thing that failed is still down. One endpoint over two commands, the way `forms` is: naming a word explains that one, naming none lists what there is to ask about — which a caller that has never met this vocabulary needs before it can name anything. Served rather than shipped; a second copy of the table in the web app would be a surface explaining a word its own way. The terminal offers it on `?`, over the words on the screen. |
| `stuck` | `/api/stuck` | dashboard, partial | Served on the web, which is where the dashboard's own "N stuck" figure lands: each entry is named the way `/api/trace` is asked, so the count leads somewhere. The terminal's panel lists them and offers no way to follow one. |
| `seed` | `seed` | none | Offered on the web. No terminal form, like every other write. |
| `adopt` | `adopt` | none | As above. |
| `reset` | `reset` | none | As above. It is the one write in this group that destroys work, which makes the terminal's silence about it the least costly silence in the table. |
| `backup` | `backup` | none | Reachable in full. It takes the one thing the command line takes — the single service to capture instead of the whole stack — and takes no path at all, because a capture goes into the backups directory lemonfiber chose, whoever asked for it. Refusing a stack that cannot be proven stopped is the command's own rule now rather than the command line's, so a browser cannot ask for the live-database capture a shell was never allowed either. No terminal form, like every other write. |
| `support` | `support`, partial | none | Everything that decides what the bundle holds is offered: whether to produce one at all, how much of each service's log to take, whether media filenames survive, which settings are shown as they are, and the agreement that showing one takes. Where it goes is the one thing that is not — `--out` names a path on the host and a browser has none to name — so a bundle asked for here is written with lemonfiber's own files, under a name carrying the moment it was taken. Nothing leaves the machine on either surface. |
| `ui` | intrinsic | none | **The one honest exception in this table.** A surface cannot start itself: the request either reaches a server that is already serving, where it means nothing, or it means starting a second server, which is a different request — and it would make a running server hand out the per-run token for a new one. Unbuilt rather than excepted on the terminal, where a key that starts the web surface and prints its address is meaningful. |
| `restore` | `restore`, partial | none | Offered, and the confirmation is inside the command rather than in a screen: unconfirmed it verifies the archive and answers with what it holds, having touched nothing, and only a second request carrying the agreement overwrites — so a surface that skipped the listing would be asking for something else, not rendering the same thing differently. Accepting a re-point is offered. Restoring an archive from anywhere on the host is not: a browser names one of the backups this machine took and the name is resolved beneath the backups directory, so a name carrying a path, or climbing out of that directory, is refused by name rather than followed. |

## What the table adds up to

Of the twenty-six requests, sixteen reach the web in full, seven reach it in
part, two do not reach it at all, and one — `ui` — is an honest exception. Nine
gaps and one exception is the split `G1-R1` asks for, and it is deliberately
lopsided: an exception has to survive being argued, and almost nothing does.

These four numbers are read back from the table above by the guard, because a
version of this paragraph said ten and five where the rows said eleven and four,
and a summary nobody checks is how a page that exists to be counted stops being
countable.

The other three exceptions the spec names run the other way — a live-refreshing
dashboard and an open event stream have no command-line form, and `--json` has no
meaning on a screen — so none of them is a row here. This table reads from the
command line outwards.

Three arguments were made and did not survive, which is worth recording so they
are not made again:

**Backup, restore and support are not web exceptions.** The appeal is that a
browser cannot choose a path on the host. It cannot, but the operation does not
need it to — the server is on the host and runs as the operator. The picker is
poorer on the web; the action is not unsuited to it. All three are offered now,
and what building them took from that argument is the other half of it: a path
the server can write is a path a browser must not supply. A capture takes none, a
bundle goes where lemonfiber keeps its own files, and a restore reads one of the
archives in the backups directory by the name it was written under.

**A long or streaming action is not a different action.** `logs --follow` and
`watch` run for minutes and produce lines rather than a value, and the web already
has both halves of the answer: a job name for work that outlives a request, and an
event stream for what it says while it runs. Delivery differs; the request does
not.

**A read is not exempt from a requirement about actions.** Reading is most of what
an operator asks for, and a surface that could not say what is running would be
crippled in exactly the way parity exists to prevent. So `version`, `forms`,
`stuck`, `trace` and `explain` were counted alongside the writes. Being reads makes
them cheap to build, not optional to build — and cheap was the argument for doing
them first: each of the four served before it was one endpoint over a command the
core already carried out. `explain` was the one left, and the one with no command to
reach: the command line read the table compiled into the binary itself. So it was
given one — rather than a second reader of that table beside the first, which is what
would have made the endpoint a surface with behaviour of its own instead of a read
like the others.

## The terminal acts on five and asks about six

It did neither. The dashboard read and the log viewer read; the wizard was the
only screen that changed anything, and it only runs on a machine that has not been
set up yet. The argument for leaving it that way was that the shell is right there —
an operator reading `sonarr: unhealthy` can type the restart. It is true and it
proves too much: by the same reasoning the web needs no actions either, since the
operator could open a terminal. The operator this surface exists for is the one on
the far end of a remote session, who is the least able of the three to reach
another surface to act on what this one has just told them.

The dashboard now offers the five the screen already showed state for — starting,
stopping, switching, restarting and fetching — and answers the six reads it showed
nothing of: the versions in play, the forms the stack declares, the settings, the
quality in force, what the household asked for, and where one of those things got
to. Nine of the twenty-six requests still have a terminal form of `none`, which is
what the rest of this gap looks like now.

**The action is the web's action.** A key names one of the actions
[`actions.rs`](../../crates/lemonfiber-api/src/actions.rs) offers and that name is
put through the same translation a browser's request goes through, so the terminal
reaches the command a browser reaches and cannot grow one a browser has no form of.
Which of them refuse an empty list of forms is asked of that table rather than
written down again, so the two surfaces cannot come to disagree about it. What the
screen decides — which key, which subject, which question, the question in front of
an action and how an answer is moved through — is in
[`acting/`](../../crates/lemonfiber/src/acting.rs), not in the terminal file, which
is the one this workspace deliberately does not test.

**The question is the web's read.** A question is held by the path the web serves
it at, and that name goes through
[`reads.rs`](../../crates/lemonfiber-api/src/reads.rs) — the same table the
endpoints themselves go through — so a question asked at this screen reaches the
command a browser reaches. What a question must be given before it can be asked is
that table's too: a trace with nothing typed is refused in the sentence a browser is
refused with, rather than in one this screen wrote. The settings are a read like the
others and are withheld like the others: the screen asks for `config show` and never
opens the file, which a guard beside the withholding list refuses to let it start
doing.

**Six questions, one key.** The screen already answered `q`, `r`, `?` and five
actions, and a key per request does not survive being done twice — twenty-six
requests will not fit on one row of a footer, let alone in anybody's memory. So one
key opens the list of what this stack can be asked, which is the same list, the same
movement and the same enter an action's own subjects are chosen with. A seventh
question goes on that list without costing anybody a letter to learn.

**An answer is read rather than glanced at.** Every setting a stack declares is
dozens of lines and a trace is a season at a time, so the box moves through the
answer instead of showing the first of it and counting the rest — and it says what
is off each end, because either end of a long answer looks exactly like a short one.
It is a box over the dashboard rather than a screen of its own: what an operator came
to this surface for is the panels behind it, which go on gathering the whole time, so
one key gives back a screen that is current rather than one that has to be caught up.

**Nothing happens on one keypress.** A key opens the list of what the action can be
given; taking one puts the question, which names what is about to happen and to
what; only an explicit yes goes ahead, the way the teardown's own question is read.
On a screen where one finger reaches a teardown that is the difference between an
action and an accident, and it is also where the operator is told what the action
reaches before it reaches it.

**A long action reports through the screen it interrupted, and is left rather than
stopped.** The web answers an action that reaches the container engine with a job's
name, because a request cannot be held open for minutes. A terminal needs no such
indirection: the dashboard is already the report. The panels go on gathering every
second while the work runs, so a restart shows as the services going down and coming
back in the panel that lists them, and nothing is drawn over that — only the footer
says what is running. Leaving stops this screen waiting; what the container engine
was already asked to do is between the operator and the engine, exactly as a closed
browser tab takes nothing with it. What the action came to is shown when it lands,
in the words the command line gives for the same run.

## What is missing beside the table

One thing still weakens "reachable from the web" for every long-running action, and
it is not a row here because it is not a request:

- **Narration carries no job's name.** `events::saying::Saying` puts a wait's own
  words on the stream, and the context it says them through belongs to the run
  rather than to a job, so two actions running at once are narrated into one
  undifferentiated line of talk. What each one *came to* is asked for by name at
  `/api/jobs/<name>`; what it is saying on the way there is anonymous.

## Related

- [dispatch.md](dispatch.md) — the one entry point all three surfaces go through
- [module-layout.md](module-layout.md) — where each surface's code lives
- [G1 interface tiers](https://github.com/lemonfiber/spec/blob/main/10-functional/features/g-ux/g1-interface-tiers.md) — the requirement and its four exceptions
- [web-api contract](https://github.com/lemonfiber/spec/blob/main/20-architecture/contracts/web-api.md) — the converse rule: the web exposes nothing the command line cannot do
