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
| Web, writes | [`actions.rs`](../../crates/lemonfiber-api/src/actions.rs) | `OFFERED`, one name per action |
| Web, reads | [`read.rs`](../../crates/lemonfiber-api/src/read.rs) · [`setup.rs`](../../crates/lemonfiber-api/src/setup.rs) · [`jobs.rs`](../../crates/lemonfiber-api/src/jobs.rs) | the routes each declares |
| Terminal | [`terminal.rs`](../../crates/lemonfiber/src/terminal.rs) | the keys the loop reads |

The table below is checked against the first three by
[`surface_parity.rs`](../../crates/lemonfiber/tests/surface_parity.rs): a request
with no row fails, a row naming an action or a route that does not exist fails,
and an action or a route the web offers that no row accounts for fails. A route
that answers no command-line request at all — the stream, the path actions are
asked for through, and the one a job's name is redeemed at — is declared there
by name, so adding a route is a decision somebody makes rather than one that
happens. The terminal column is prose — its keys live in the one file this
workspace deliberately does not test — so it is the one column a reader has to
check by eye.

## The table

**Web** names the actions and routes that reach a request, or `none`, or
`intrinsic`. **Terminal** names the screen that offers it, or `none`, or
`intrinsic`. Either may add `partial`, which means some of the request is reachable
and the rest is named in **Standing**.

| Request | Web | Terminal | Standing |
|---------|-----|----------|----------|
| `setup` | `/api/setup`, `/api/setup/answer`, `/api/setup/next`, `/api/setup/back`, `/api/setup/apply`, partial | wizard | Completable from a browser and from a terminal. Two affordances are terminal-only — proving a credential against the provider while the answer is being given, and choosing how to recover an interrupted apply — and neither blocks finishing setup. |
| `version` | none | none | Unbuilt, and the cheapest gap on this page: one read, no arguments, an answer the core already renders. |
| `forms` | `/api/forms` | none | Served on the web, and through one endpoint because the command line spells it as one request: naming no form lists what the stack declares, naming some says what starting those would come to. The profile carried on `/api/services` is a Compose profile and not a form, and neither list contains the other, so this needed an endpoint of its own rather than a reading of that one. The terminal has no way to ask what a stack declares. |
| `up` | `up`, partial | none | Starting a form is offered. Starting only some of its services is not reachable at all: `--service` never reaches a `Command` — the command line runs its own streamed start around it — so there is nothing to hand them to, and services named to this action are refused rather than dropped for a start of the whole form. Whether that is its own request, the way `Halt` is not `Down`, is a question for the core. The terminal shows a service is down and offers nothing to do about it. |
| `down` | `down`, partial | none | The teardown is offered, and so is stopping named services, which is `Command::Halt` rather than an argument to this one. Letting anything still downloading finish first is not: `--wait` is a loop the command line runs around a queue reading before it asks for the stop, not something `Command::Down` carries, so a browser can only stop now. Its companion `--yes` answers a prompt no machine-readable run is put, and needs no web form. |
| `switch` | `switch` | none | Reachable in full. It refuses an empty `forms`, and `/api/forms` serves the names — which is what it was waiting on. No terminal form, like every other write. |
| `restart` | `restart` | none | As `switch`: it refuses an empty `forms`, which `/api/forms` serves. |
| `pull` | `pull` | none | As `switch`: it refuses an empty `forms`, which `/api/forms` serves. |
| `ps` | `/api/status`, `/api/services` | dashboard | Reachable from all three. The dashboard and the endpoints are fed by the same gather. |
| `logs` | `/api/logs`, partial | viewer | The scrollback is a read and is served. Following is not: the event stream carries the dashboard gather and the narration a wait produces, never a service's own lines, so a browser cannot watch them arrive. `--watch` is the terminal's own rendering of the same lines, not a separate request. |
| `config` | `config-set`, partial | none | Changing a setting is offered; reading one is not. There is no endpoint behind `config get` or `config show`, so a browser can write a value it cannot read back. |
| `quality` | `quality-set`, `quality-reapply`, `quality-upgrade`, partial | none | Every write is offered and the read is not: `quality show` — the preset in force, what each one means, and what it costs — has no endpoint, and it is the screen a browser is best at. |
| `doctor` | `/api/checks`, `/api/storage`, partial | dashboard, partial | The diagnosis is served. Repair is not: `--fix`, `--yes`, `--fix-disruptive`, `--undo` and `--accept` have no web form, and an offer-and-consent flow is something HTML does better than a terminal prompt. The dashboard shows storage and VPN facts the diagnosis also reads, without being the diagnosis. |
| `watch` | none | none | Unbuilt. It is long-running work that ends in one report, which is exactly the shape the web already answers with a job name — and a guard started from a browser and left running is the useful case, not the awkward one. What it needs that a command does not is a way to stop it, since there is no terminal to interrupt. |
| `trace` | none | none | Unbuilt. A read, and the half of a screen that already half-exists: `/api/requests` reports what the household asked for, and following one item is what a reader does next. |
| `household` | `/api/requests` | none | Served on the web. The terminal has no view of what the household asked for. |
| `walkthrough` | none | none | Unbuilt, and the surface it is least built for is the one it was designed for: it narrates for minutes, which is a job plus the event stream, and its audience is a first-time operator who is likelier to be in a browser than in a shell. |
| `explain` | none | glossary | Answered from a table compiled into the binary, so it needs neither a stack nor a daemon. The terminal offers it on `?`. The browser has no route to it, and shipping a second copy of the glossary into the web app would be a surface implementing behaviour of its own. |
| `stuck` | none | dashboard, partial | Unbuilt on the web, which is where the dashboard's own "N stuck" figure most wants somewhere to land. The terminal's panel lists them and offers no way to follow one. |
| `seed` | `seed` | none | Offered on the web. No terminal form, like every other write. |
| `adopt` | `adopt` | none | As above. |
| `reset` | `reset` | none | As above. It is the one write in this group that destroys work, which makes the terminal's silence about it the least costly silence in the table. |
| `backup` | none | none | Unbuilt on both. Not an exception: the server runs on the host as the operator, so a path typed into a form is a path it can write. What a browser cannot do is browse to one. |
| `support` | none | none | Unbuilt on both, and the same argument as `backup`. The bundle is written where it is produced and sent nowhere, so the only web-specific question is which path, not whether. |
| `ui` | intrinsic | none | **The one honest exception in this table.** A surface cannot start itself: the request either reaches a server that is already serving, where it means nothing, or it means starting a second server, which is a different request — and it would make a running server hand out the per-run token for a new one. Unbuilt rather than excepted on the terminal, where a key that starts the web surface and prints its address is meaningful. |
| `restore` | none | none | Unbuilt on both, and the same argument as `backup`, sharpened: choosing the wrong archive cannot be taken back, so the missing part is not the write but the confirmation of what is about to be overwritten. |

## What the table adds up to

Of the twenty-six requests, nine reach the web in full, seven reach it in part,
nine do not reach it at all, and one — `ui` — is an honest exception. Sixteen
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
poorer on the web; the action is not unsuited to it.

**A long or streaming action is not a different action.** `logs --follow` and
`watch` run for minutes and produce lines rather than a value, and the web already
has both halves of the answer: a job name for work that outlives a request, and an
event stream for what it says while it runs. Delivery differs; the request does
not.

**A read is not exempt from a requirement about actions.** Reading is most of what
an operator asks for, and a surface that could not say what is running would be
crippled in exactly the way parity exists to prevent. So `version`, `forms`,
`stuck`, `trace` and `explain` were counted alongside the writes, and `forms` is
served. Being reads makes them cheap to build, not optional to build — and cheap is
the argument for doing them first: four of the nine requests absent from the web
are one endpoint each over a command the core already carries out.

## The terminal offers no action at all

This is the largest single gap and it is not an exception. The dashboard reads and
the log viewer reads; the wizard is the only screen that changes anything, and it
only runs on a machine that has not been set up yet. Twenty of the twenty-six
requests have a terminal form of `none`. Of the six that do not, five are reads
and the sixth is the wizard.

The argument for leaving it that way is that the shell is right there — an operator
reading `sonarr: unhealthy` can type the restart. It is true and it proves too
much: by the same reasoning the web needs no actions either, since the operator
could open a terminal. The operator the terminal surface exists for is the one on
the far end of an SSH session, who is the least able of the three to reach another
surface to act on what this one has just told them.

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
