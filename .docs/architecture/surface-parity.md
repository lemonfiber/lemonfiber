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
| Terminal, writes | [`acting/offer.rs`](../../crates/lemonfiber/src/acting/offer.rs) · [`acting/errand.rs`](../../crates/lemonfiber/src/acting/errand.rs) · [`acting/lasting.rs`](../../crates/lemonfiber/src/acting/lasting.rs) · [`acting/quality.rs`](../../crates/lemonfiber/src/acting/quality.rs) · [`acting/surface.rs`](../../crates/lemonfiber/src/acting/surface.rs) | `OFFERED`, one key per action; the errands behind one more key; the two that keep going behind another; the three quality changes behind a fourth; and the key that hands the terminal over |
| Terminal, reads | [`acting/question.rs`](../../crates/lemonfiber/src/acting/question.rs) · [`terminal.rs`](../../crates/lemonfiber/src/terminal.rs) | the questions the list holds, and the screens the loop opens |

The table below is checked against all five by
[`surface_parity.rs`](../../crates/lemonfiber/tests/surface_parity.rs): a request
with no row fails, a row naming an action or a route that does not exist fails,
and an action or a route the web offers that no row accounts for fails. A route
that answers no command-line request at all — the stream, the path actions are
asked for through, and the one a job's name is redeemed at — is declared there
by name, so adding a route is a decision somebody makes rather than one that
happens.

The terminal column is read the same way now, and the join it was missing is
[`reaching.rs`](../../crates/lemonfiber/src/reaching.rs). `acting/` is `mod acting;`
in `main.rs` and private to the binary, so an integration test could never reach the
screen's own lists; what is in the library instead is a projection of them, naming
each command-line request the dashboard reaches and the action or the read it
reaches it through. It is held at both ends. `acting/`'s own tests hold every action
and every question the screen offers to an entry there, and every entry there to
something the screen offers; the guard holds every `dashboard` cell below to the
same list, in both directions — a row claiming this screen reaches a request it
does not fails, and an offer no row accounts for fails.

One kind of claim in that column is still a reader's job: the three rows naming a
screen that is not the dashboard — `wizard`, `viewer` and `glossary`. Beside them are
the three requests the dashboard's *panels* answer rather than its lists; a panel is a
rendering rather than a named request, so there is no list of them to hold a row
against, and those three are written down in the projection instead, where a reader
can at least see the claim in one place. The one request the screen reaches through no
table at all — the web surface, which no other surface has an action for — is written
down there too, and unlike the panels it is held to something: the test beside that key
presses it and asserts what it asks for.

## The table

**Web** names the actions and routes that reach a request, or `none`, or
`intrinsic`. **Terminal** names the screen that offers it, or `none`, or
`intrinsic`. Either may add `partial`, which means some of the request is reachable
and the rest is named in **Standing**.

| Request | Web | Terminal | Standing |
|---------|-----|----------|----------|
| `setup` | `/api/setup`, `/api/setup/answer`, `/api/setup/next`, `/api/setup/back`, `/api/setup/apply`, `/api/setup/recover` | wizard | Reachable in full from a browser and from a terminal. The two affordances that were terminal-only were the command line doing something around the command, not anything a browser is unsuited to. A credential is proven against its live service as the answer is given, so what is recorded is what the test established rather than what the caller claimed, and what the service did comes back on the report; a test that does not prove one is not a refusal, which leaves the three ways out a terminal run offers — enter another, enter none, or go on with one recorded as unverified. The three ways out of an apply that stopped part-way are a request of their own, put after the report has named what that apply had already written, so the choice is made by somebody who has seen it. |
| `version` | `/api/version` | dashboard | Reachable from all three, and the cheapest read on this page: no arguments, and an answer the core already renders. The dashboard asks for it off the list one key opens, and a request with no arguments has nothing left over to leave out. |
| `forms` | `/api/forms` | dashboard | Served on the web, and through one endpoint because the command line spells it as one request: naming no form lists what the stack declares, naming some says what starting those would come to. The profile carried on `/api/services` is a Compose profile and not a form, and neither list contains the other, so this needed an endpoint of its own rather than a reading of that one. The dashboard asks both halves, as two entries on its list of questions. One lists what the stack declares. The other asks that same read for that same listing and offers what it comes back with as a list to take one of, so naming a form is taking it off a list written in the stack's own words rather than spelling an id exactly — and the id handed on is the one the stack declared, which is the half a typed name would get wrong. |
| `up` | `up` | dashboard, partial | Reachable in full from a browser, and on the terminal the whole stack is a choice too, because the command carries an empty list. Starting only some of a form's services was the gap, and the question it left the core — whether that is its own request, the way `Halt` is not `Down` — is answered yes: bringing a form up creates everything its closure holds and starting named services starts the ones named, which is the same pair `down` and `stop` are, and Compose spells both pairs differently. So the action forks the way `down` already forked, reaching `Command::Start` where services are named and `Command::Up` where none are. The command line still runs its own streamed start around it, because Compose narrates for minutes and that is not a value that arrives once — but both paths resolve what they are about through the same reading of the manifest, so neither can answer about a different set of services from the other. Several forms at once are named on the dashboard now: a row of its list is marked and the marked rows are what enter takes, so the list carries what a browser sends whole. What the terminal leaves is the named services that fork, which its list has no way to name — it offers the stack's forms and not the services inside one. |
| `down` | `down` | dashboard, partial | Reachable in full from a browser. The teardown is offered on both, and several forms are torn down at once on both, the dashboard's list being marked a row at a time. Stopping named services is offered on the web and not here, which is `Command::Halt` rather than an argument to this one: the terminal's list names the stack's forms and not the services inside one. Letting anything still downloading finish first is a field on `Command::Down` now rather than a loop the command line ran around it, so a browser asks for it by saying so and is answered with a job's name, which is what a wait that can last an hour has to be answered with. What stayed on the command line is the question in front of it: it reports what stopping would interrupt, asks, and hands the answer over — and its companion `--yes` answers that prompt, which no machine-readable run is put, so it needs no web form. The wait and the named services cannot be asked for together, because what is in flight is a question about the download clients a form holds; both surfaces refuse the pair rather than dropping one of them. |
| `switch` | `switch` | dashboard | Reachable in full from all three. It refuses an empty `forms`, and `/api/forms` serves the names — which is what it was waiting on; the dashboard asks the same list of the core and offers the names it comes back with, so a stack declaring none is refused there in the same words. Several are named by marking them, and the empty list this command refuses is still not a thing the screen can send: the whole stack is offered as a row only where the translation carries one, and this action is not one of those. |
| `restart` | `restart` | dashboard, partial | As `switch` on the web, and on the terminal it takes several forms the same way. What it leaves is the named services `--service` restarts, which its list has no way to name: the list is the stack's forms, and the services inside one are a gather it does not have. |
| `pull` | `pull` | dashboard | As `switch`, and reachable in full from all three for the same reasons: it refuses an empty `forms`, which `/api/forms` and the dashboard's own list both serve, and several are named by marking them. It is the one of the five that stops nothing, and it is asked about before it runs anyway, because it can spend an hour of somebody's connection — which is the more worth asking about the more forms it was given. |
| `ps` | `/api/status`, `/api/services` | dashboard | Reachable from all three. The dashboard and the endpoints are fed by the same gather. |
| `logs` | `/api/logs` | viewer | Reachable in full. The scrollback is a read that ends, and is answered with what it read; following does not end, so it is answered with a name for the work and the lines arrive on the stream the browser already holds — one endpoint for both, because the command line spells following as a flag on this request rather than as a request of its own. The stream carries a service's own lines now, which it did not: they go down as `log`, the event name every other envelope's kind becomes, so a browser that is not following never registers for one. Nothing is dropped at the source and a browser that reads more slowly than a service speaks is let go past the window the stream carries, which is the rule already in force for every other event. `--watch` is the terminal's own rendering of the same lines, not a separate request. |
| `config` | `config-set`, `/api/config` | dashboard | Reachable in full from a browser. Writing arrived before reading, so a browser could set a value it had no way to read back; `/api/config` answers both halves of the read the way the command line spells them — naming no setting shows every one, naming one reads that one. Credentials are withheld where the settings are read rather than where they are printed, so both the endpoint and the dashboard show what `config show` prints, and a guard refuses a screen that reads the file for itself. The terminal asks both halves too, and as the same two entries: one shows every setting in a box that moves through them, and the other opens the line a trace is typed on. Naming one is not a way past the withholding — the narrowing happens after the display path rather than instead of it, so a credential asked for by name comes back withheld exactly as the listing withholds it, and the screen reaches the file for neither half. An empty name is refused in the sentence a browser is refused with rather than read as having named none, which it was not before: it reached the core as a setting to look for, matched nothing, and came back as a listing of no settings — which reads as "there is no such setting" about a setting nobody named. Changing one is offered nowhere but the browser, like every other write. |
| `quality` | `quality-set`, `quality-reapply`, `quality-upgrade`, `/api/quality` | dashboard, partial | Reachable in full from a browser. `/api/quality` serves the read the three writes were being made without: the preset in force, what each one means, and what it costs, which is the screen a browser is best at. The dashboard asks that read off its list of questions and offers all three writes behind a key of their own — not on the list of errands, because the agreement does not mean the same thing on these three as it does there. An errand that carries one answers, unconfirmed, with what it would do and changes nothing, which is what makes that run the account its question sits under; `quality-set` unconfirmed *records* the choice, and holds it only where this host would have to transcode the result in software, which is the one cost its agreement is for. A list whose rule is “unconfirmed says what it would do” cannot take an action for which that is false without the rule quietly becoming untrue for the ones it was written for. So what goes in front of each question is what that change really has to say, and the three do not have the same thing to say. Choosing is made off the four presets, each carrying what it means and roughly what an hour of it costs, because the run that would otherwise state the consequence is the run that records it — and a choice that comes back held rather than recorded puts the core's own caution on the screen with a second question under it, which is where the agreement goes on. Upgrading states what it would cost per media type and triggers nothing, which is the errands' own reading arriving where it fits. Re-asserting has nothing to say first and is not given a preamble for symmetry: it carries no agreement, and the core's report-only half of it is behind `--dry-run`, which is a property of a run rather than of a request, so no surface's action can ask for one. Which of the three carries an agreement is read from the same table the errands read it from. What the terminal leaves is the media type a choice applies to: the whole library is what the dashboard sets, and narrowing to television or film — or to music, which is an audio format rather than a resolution and forks inside the same action — is taken to a browser or a shell. |
| `doctor` | `/api/checks`, `/api/storage`, `diagnose`, `repair`, `undo`, `accept` | dashboard, partial | Reachable in full from a browser. The diagnosis is served, and so is putting it right. `repair` is the offer and the consent in one action because they are one request read twice: unconfirmed it answers with what each repair would do and what else changes if it does, and confirmed it carries out what was agreed to — named by the offer it was read in, so an answer cannot be spent on an offer that has moved on since it was read. Confirmed while naming no offer is the standing consent `--yes` spells, which is a decision taken before there was anything to read rather than a way past being told. `undo` is its own action and carries no subject at all, the core deciding which repair was last and what reversing it takes; `accept` answers a warning. What was left was the diagnosis including the checks that disturb a running system, asked for on its own — those ran here only while repairing and while answering a warning — and the argument that kept it out is what settled it: a read that disturbed something would not be a read, so this is not one. `diagnose` is the same request at the door changes are asked for. It is the surface that forks and not the core: `Command::Doctor` already carries the widening as a field, exactly as the command line spells it as a flag, so unlike `Halt` beside `Down` there is no second command to add — the checks are the same checks, reporting a real verdict where an ordinary run reports them unverified. What differs is the method, and it has to: a `GET` that took the tunnel away to prove the killswitch comes back would be a `GET` that stopped somebody's downloads. The widening is required rather than defaulted, because a request without it would be `/api/checks` under a second name. The narrowing `--only` takes is offered beside it, because both disturbing checks name what to run in what they tell the operator — the release search says `--only services.releases --disruptive` outright — and a browser that could only ask for all of them would have to drop the tunnel to spend one indexer search. It is answered with a job's name, which is what a run bounded by how long it may hold the tunnel away has to be answered with. The dashboard shows storage and VPN facts the diagnosis also reads, without being the diagnosis. |
| `watch` | `watch` | dashboard | Reachable in full, and it is the command with no ending of its own: it holds until the data location is lost, which on a machine where the drive stays put is never. So it is answered with a job's name like every other long action, and the name is also how it is stopped — a browser has no interruption to send, so releasing the name is its Ctrl-C, and what the container engine was already asked to do goes on exactly as it does when a terminal is closed. A tab closing stops nothing, which is the useful case. What bounds an abandoned one is that asking what became of it renews it: a guard nobody has asked about across two sweeps of half an hour is let go and says so, so one started on a Friday does not outlive the day. Nothing else is leased, because everything else ends by itself. The dashboard offers it behind the key that opens the two that keep going, and it is the one thing on that screen offered an end — which is the same answer, asked of the same table: a terminal's interruption is what a released name stands in for, and on a screen in raw mode that interruption is a keypress rather than a signal, so escape lets the guard go and says that nothing was stopped with it. Leaving the screen leaves it guarding, and the line on the way out says it will not end by itself and that Ctrl-C ends it. It is chosen its forms off the same list an action's own subjects are chosen from and by the same movement, so one guard covers several forms at once; the whole stack is not among the choices at all, because the command refuses a guard with nothing to stop and the list is built by offering each subject to that refusal. |
| `trace` | `/api/trace` | dashboard, partial | Served on the web, and it completes a screen that half-existed: `/api/requests` reports what the household asked for, and this follows one of them. What to follow is one query parameter — the command line takes it as words so it can be typed unquoted, and a query string carries the title whole. It is the one question the dashboard has to be given something before it can ask, so taking it opens a line to type the title on, and an empty one is refused in the sentence a browser is refused with. Narrowing to one season is not offered there: it is a second thing to type for an answer that already reads season by season. |
| `household` | `/api/requests` | dashboard | Served on the web, and asked off the dashboard's own list of questions. Narrowing to one member is offered there now, as a second entry on the same list: taking it opens the line a trace is typed on, and what is typed fills the member a query string carries. Typed rather than taken off a list of the household, because the list it would be taken off is the answer the narrowing exists to avoid asking for — which is the difference between this and the two that are picked. An empty name is refused in the sentence a browser is refused with, which it also was not before: it matched nobody and came back as a household that has asked for nothing, the one reading this report is written to refuse. |
| `walkthrough` | `walkthrough` | dashboard | Reachable in full, on the surface it was designed for: a first-time operator is likelier to be in a browser than in a shell. It is a job plus the stream — the report at the end is what the name is redeemed for, and each step goes down the stream the moment it is true, because a walk read back afterwards is a report and the operator would have learned what happened rather than watched it happen. The step goes down whole rather than as a sentence: the words are the core's own, and rendering them into a line for the browser would be a second copy of the walk's prose beside the one the terminal draws. Naming nothing is a request rather than an omission, and is offered as one. Reachable in full from the dashboard too, behind the same key: what to look for is typed on a line of its own and naming nothing there is the same request it is everywhere else, and the steps arrive in the box as they become true. They are drawn by the renderer a shell reaches for the same step, so there is one account of a walk on this surface and not two — and what it came to goes under the steps that were watched, which is the order a shell shows them in. It ends by itself, so the screen offers no end for it and leaving waits for it, as leaving waits for every other action. |
| `explain` | `/api/explain` | glossary | Reachable in full, and the one read that needs neither a stack nor a daemon: the words are a table compiled into the binary, so a browser meeting one in a failure can ask what it means while the thing that failed is still down. One endpoint over two commands, the way `forms` is: naming a word explains that one, naming none lists what there is to ask about — which a caller that has never met this vocabulary needs before it can name anything. Served rather than shipped; a second copy of the table in the web app would be a surface explaining a word its own way. The terminal offers it on `?`, over the words on the screen. |
| `stuck` | `/api/stuck` | dashboard | Served on the web, which is where the dashboard's own "N stuck" figure lands: each entry is named the way `/api/trace` is asked, so the count leads somewhere. The terminal reaches it as a question now and follows one the same way the web's list does: taking the question asks that read, and the entries it comes back with are offered as a list to take one of, which asks `/api/trace` by that entry's own title. Taken rather than typed, because the title is on the screen already and retyping it is a spelling test. The panel stays and is not this: it renders the queue-health gather, which counts what has stopped and names the cause where several items share one, rather than naming each item the way a trace is asked for — two readings of one worry, and it was the second the screen could not reach. |
| `seed` | `seed` | dashboard | Reachable from all three. It is the first of the six errands the dashboard keeps behind one key, and the one with nothing to leave out: the command takes no arguments, so the question names what wiring the services to each other comes to and an explicit yes is the whole of what it needs. |
| `adopt` | `adopt` | dashboard | As above, and the pair it makes with `reset` is what the errands are ordered by. Keeping what an operator changed sits near the top of that list and throwing it away sits at the bottom of it, so nobody lands on the destructive one by pressing enter at a list they have only just opened. |
| `reset` | `reset` | dashboard | Reachable in full on both. It is the one write in this group that destroys work, so the terminal does not put the question until the stack has said what would be lost: the unconfirmed command goes first, the reverts it names are what the box shows, and the question sits under that account rather than over it. An effect somebody reads after agreeing to it is not one they agreed to, which is the reading every repair is already offered under. The agreement itself is the command's own field, so the screen carries no second idea of what confirming means. |
| `backup` | `backup` | dashboard, partial | Reachable in full from a browser. It takes the one thing the command line takes — the single service to capture instead of the whole stack — and takes no path at all, because a capture goes into the backups directory lemonfiber chose, whoever asked for it. Refusing a stack that cannot be proven stopped is the command's own rule now rather than the command line's, so a browser cannot ask for the live-database capture a shell was never allowed either. What the terminal leaves is that single service: the names are a gather this list does not have, and a line to type one on would be a name nothing checked before the capture ran. |
| `support` | `support`, `/api/bundle/{name}` | dashboard, partial | Reachable in full from a browser. Everything that decides what the bundle holds is offered: whether to produce one at all, how much of each service's log to take, whether media filenames survive, which settings are shown as they are, and the agreement that showing one takes. Where it goes is offered in the only terms a browser has — `--out` names a path on the host and a browser has none to name, so it is handed the file instead. A bundle asked for here is written with lemonfiber's own files under a name carrying the moment it was taken, and that name is what it is fetched back by. It is the better half of the trade rather than the poorer: a host path is useless in the case a browser is most used for, which is lemonfiber running on a machine nobody is sitting at. The name is resolved beneath the bundles directory by the core and never followed as a path; the fetch carries the token every other request carries; and the only destination that writes into that directory is the one this surface asks for — so what comes back is what this surface produced, under the agreement that produced it, and never a file the reads beside it withhold. The terminal offers the half that decides whether there is a file at all and none of the four that decide what is in one: it asks what a bundle would hold, shows the answer, and writes one only if that is agreed to, with every careful default left where it is — the ordinary window of log lines, media filenames replaced, nothing revealed. Which settings are shown as they are is deliberately the one thing not offered anywhere but the browser: a terminal-only way past the withholding list would be a surface showing a credential no other surface shows. Nothing leaves the machine on any of the three: the bytes go to the browser that asked, over the loopback connection it already holds. |
| `ui` | intrinsic | dashboard, partial | **The one honest exception in this table.** A surface cannot start itself: the request either reaches a server that is already serving, where it means nothing, or it means starting a second server, which is a different request — and it would make a running server hand out the per-run token for a new one. It is not the exception on the terminal, where a key that starts the web surface and prints its address is meaningful — and it is built now. It is a key of its own rather than an entry on either list, because it is the one request on that screen that is not work sent to the stack: it reaches no action and no read, since no other surface has one to reach, and what it does is hand the terminal over. So it ends the screen rather than sharing it — the address, the warning that the connection is not encrypted and the token every request must carry are eleven lines somebody has to read and copy, which a box on a terminal in raw mode cannot offer and a torn-down alternate screen would take away. What the terminal leaves is the three choices `--port`, `--no-browser` and `--assets` make, none of which has anywhere to be made on a dashboard: a port typed at a screen would be a number nothing had checked was free, and the address that was bound is printed either way. |
| `restore` | `restore`, `/api/backups` | dashboard, partial | Reachable in full from a browser, and the confirmation is inside the command rather than in a screen: unconfirmed it verifies the archive and answers with what it holds, having touched nothing, and only a second request carrying the agreement overwrites — so a surface that skipped the listing would be asking for something else, not rendering the same thing differently. That is what makes the listing the terminal's consequence-before-the-question: what the archive would overwrite is on the screen, and the question is the line under it. What was missing was the step before naming one: the name is typed rather than pointed at, and nothing anywhere said what this machine had kept, so a browser could only ask for an archive whose name it already knew. `/api/backups` is that listing, and on the command line it is `lemonfiber restore` with nothing named — the fork `forms` and `explain` take on their own word, taken here for the reason they take it: a surface that has to name a thing cannot know the names in advance. The name still travels as a name and is resolved beneath the backups directory by the core, so one carrying a path, or climbing out of that directory, is refused by name rather than followed — which is the poorer picker `G1` records, not a request a browser cannot make. Accepting a re-point is offered on the web and not here, so a restore onto a different data root is refused at this screen and taken to a browser or a shell. |

## What the table adds up to

Of the twenty-six requests, twenty-five reach the web in full, zero reach it in
part, zero do not reach it at all, and one — `ui` — is an honest exception. Zero
gaps and one exception is the split `G1-R1` asks for, and it is deliberately
lopsided: an exception has to survive being argued, and almost nothing does.

**The web column is finished.** Nothing is out of a browser's reach and nothing is
half in it: every request either arrives whole or is the one that cannot arrive at
all. The last row to close was the diagnosis that disturbs a running system, and it
closed by taking the argument against it seriously rather than by weakening it — a
read that disturbed something would still not be a read, so it is asked for at the
door changes are asked for instead. That leaves `ui` as the only exception this
table permits, in either column.

On the other side of the table sixteen reach the terminal in full, ten reach
the terminal in part, and zero have no terminal form — which is where three stood
three slices ago, nine before the one ahead of that, and twenty before the one that
began it. Every request the command line accepts is reachable from a screen as
well. What that column still loses is an argument at a time, on every row reading
`dashboard, partial`, and those are gaps on the same terms as the web's — a screen
that reaches a request but not one of its arguments has not finished reaching it.
Ten rows read that way, and with the web column finished they are the whole of
what stands between this table and `G1-R1`.

Three of those rows closed at once because they were one gap wearing three names.
`switch`, `pull` and `watch` were each short of nothing but a list that could name
several of the stack's forms, and the list names several now. `up`, `down` and
`restart` lost that same argument and keep a different one: the named services
their command line spells with `--service`, which a list offering the stack's
forms has no way to name.

Four more closed at once for the same kind of reason: a screen that could show a
list and not name one thing in it. A setting and a member of the household are
named by typing, on the line a trace was already typed on. A form and one of the
items whose download has stopped are named by taking one off a listing the screen
asks for first — that question's own read given nothing, so no listing is written
down twice. Which of the four is which is not a matter of taste: a form and a stuck
item are already written down somewhere the screen can read them, where a setting
or a member could only be picked off the very answer the narrowing exists to avoid
asking for.

A row can also lose an argument without any figure moving, and `quality` is the
case in point: it read `dashboard, partial` while the dashboard offered none of
its three writes, and reads the same now that it offers all three, because the
media type a choice applies to is left to a browser. That is the shape of the rest
of this column's work — the counts say how many requests are reached and how many
only in part, and the rows say which argument each of the partial ones is short of.

These eight numbers are read back from the table above by the guard, because a
version of this paragraph said ten and five where the rows said eleven and four,
and a summary nobody checks is how a page that exists to be counted stops being
countable. Both columns are counted the same three ways now. The terminal column
had only the figure saying how many requests reach no screen at all, and that
figure reached zero — so a column whose rows were still losing an argument each
read as finished, and every slice closing one of them moved a number nothing was
reading.

The other three exceptions the spec names run the other way — a live-refreshing
dashboard and an open event stream have no command-line form, and `--json` has no
meaning on a screen — so none of them is a row here. This table reads from the
command line outwards.

Five arguments were made and did not survive, which is worth recording so they
are not made again:

**Backup, restore and support are not web exceptions.** The appeal is that a
browser cannot choose a path on the host. It cannot, but the operation does not
need it to — the server is on the host and runs as the operator. The picker is
poorer on the web; the action is not unsuited to it. All three are offered now,
and what building them took from that argument is the other half of it: a path
the server can write is a path a browser must not supply. A capture takes none, a
bundle goes where lemonfiber keeps its own files and is then handed to the browser
that asked for it, and a restore reads one of the archives in the backups directory
by the name it was written under — off a listing this surface serves, because a name
a browser has no way of being told is a name it has no way of using.

**A long or streaming action is not a different action.** `logs --follow`,
`watch` and `walkthrough` run for minutes and produce lines rather than a value,
and the web already had both halves of the answer: a job name for work that
outlives a request, and an event stream for what it says while it runs. Delivery
differs; the request does not. All three are offered now, and what building them
took from that argument is the half it did not state: a job name is only the whole
answer if it can be given back. A terminal ends what it is running by interrupting
it and a browser has nothing to interrupt with, so the name it was answered with is
the handle — released, and the work ends where a shell's own interruption would
have left it. That is a property of the name rather than of any one of the three,
which is why it is one verb on the job rather than three actions.

**A conversation is not a terminal's alone.** Setup proved a credential while the
answer was being given, and offered three ways out of an apply that stopped
part-way, and this page recorded both as things only a terminal did. Neither is
about a terminal. Proving a key is a live test the server runs — the server is on
the host, as it was for the path argument above — and a browser is the better place
to watch one happen than a line that scrolls past. Choosing how to recover is an
offer-and-consent flow, which is the thing HTML does better than a prompt; and the
command line's own refusal to make that choice for a piped run is the proof, because
a surface that can only ask interactively has not built a request, it has built a
prompt. Both are offered now, and what building them took from the argument is that
neither could be moved without moving what it decided into the command first.

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

**A door is not a request.** The checks that disturb a running system reached the
web attached to the read that serves the diagnosis, and the reason they were not
offered was true of that read: a `GET` that took the tunnel away to prove the
killswitch would be a `GET` that stopped somebody's downloads. What did not follow
was the conclusion. "This cannot be served here" and "this cannot be served *at
this door*" are the same sentence with one clause removed, and only the first would
have been an exception — the second is a door, and this surface already had the
other one. So the checks that disturb are asked for where every other change is
asked for, and the diagnosis that disturbs nothing is still read where reads are.
That is not two ways to ask one thing; it is the one distinction the requirement
turns on, which is between looking and touching. What building it took from the
argument is that the fork belongs to the surface: the core spells this as a field
on the command it already had, so a second command here would have been a browser
asking for something no command line can.

## The terminal acts on eleven and asks about six

It did neither. The dashboard read and the log viewer read; the wizard was the
only screen that changed anything, and it only runs on a machine that has not been
set up yet. The argument for leaving it that way was that the shell is right there —
an operator reading `sonarr: unhealthy` can type the restart. It is true and it
proves too much: by the same reasoning the web needs no actions either, since the
operator could open a terminal. The operator this surface exists for is the one on
the far end of a remote session, who is the least able of the three to reach
another surface to act on what this one has just told them.

The dashboard offers the five the screen already showed state for — starting,
stopping, switching, restarting and fetching — and answers the six reads it showed
nothing of: the versions in play, the forms the stack declares, the settings, the
quality in force, what the household asked for, and where one of those things got
to. Beside them are the six writes that are not about what is running at all: the
wiring, keeping an operator's edits, throwing them away, a capture, a bundle and an
archive put back; the two that keep going once they are started; the key that hands
the terminal to the web surface; and the three changes to the quality the stack aims
for, which sit apart from the errands because what an agreement means on them is not
what it means there. No request the command line accepts has a terminal form of
`none` any more.

**The action is the web's action.** A key names one of the actions
[`actions.rs`](../../crates/lemonfiber-api/src/actions.rs) offers and that name is
put through the same translation a browser's request goes through, so the terminal
reaches the command a browser reaches and cannot grow one a browser has no form of.
An errand off the list the second key opens names one the same way, and what it may
be given is that table's answer too — including which of them carry the operator's
agreement, which is what decides the three that say what they would do first.
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

**Six questions, one key — and six errands behind another.** The screen already
answered `q`, `r`, `?` and five actions, and a key per request does not survive being
done twice: twenty-six requests will not fit on one row of a footer, let alone in
anybody's memory. So one key opens the list of what this stack can be asked, and one
more opens the rest of what it can be told to do — the same list, the same movement
and the same enter an action's own subjects are chosen with. A seventh of either goes
on its list without costing anybody a letter to learn.

The second key says only `more`, which is the most it can honestly say. A wiring, a
capture, a bundle, an archive put back and a revert have no one word between them
that is not vaguer than the six names on the list, and a key claiming something the
list does not hold is worse than one claiming nothing. What each errand is is said on
its own row, where there is room to say it.

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

**A list of the stack's own forms names several of them.** The command line takes a
list of forms and a browser sends one whole; this list took one, and that was the
last thing four of the five lifecycle actions and the guard were short of. A row is
marked with the space bar and the marked rows are what enter takes, and where
nothing is marked enter takes the row under the cursor — so an operator who never
presses the new key has the screen they had before it existed. The line under the
list says which of those two enter would do and changes when that changes, because
the one moment the choice is ambiguous is the moment the screen can resolve it, and
a rule somebody has to remember is a rule they will get wrong once, on the teardown.
Marking is drawn in the row rather than kept in a legend, and a list that takes one
— the questions, the errands — has nowhere to put a mark, which is how it says it
takes one.

**The whole stack is instead of naming forms, not one more of them.** Marking it
takes the marks off the forms and marking a form takes the mark off it: what the two
together would send is what the whole stack alone would send, and a list showing
both marked would be naming something it was not about to send. The marks move where
the operator can watch them move, which is the whole of how that rule is taught.
Naming nothing at all is not a state this screen reaches — the cursor is always on a
row — so an empty list of forms reaches the core only from the row that says
`everything`, and that row is offered only where the translation carries one. The
two actions that read an empty list as the whole stack and the three that refuse it
stay exactly as far apart here as they are in the table both are asked of, rather
than being blurred into one keypress meaning two things. Several forms go through
that same translation as one list, so what is sent is the command a browser sends,
and the question in front of it names every form it covers rather than counting
them: agreeing to a teardown of four forms is agreeing to four names.

**And where the consequence is larger than a sentence, it is read before it is
agreed to.** Three of the errands have a half that reports and changes nothing —
what a reset would revert, what a bundle would hold, what an archive would overwrite
— and those three send that half first. Its answer is what the box shows, and the
question is the line under the answer rather than a sentence in front of it. An
effect somebody reads after agreeing is not one they agreed to, which is the reading
each repair `doctor --fix` offers is already put under. Which three is asked of the
same table that says which actions carry the operator's agreement, rather than
decided again here — the place two lists would come to disagree is in front of
somebody about to throw work away.

**A name, and never a path.** The one errand that has to be given something is the
restore, and what it takes is the name a backup was written under, typed on a line
of its own. Nothing on any surface lists what this machine has kept, so a browser
types one too; what the terminal cannot do is name a *file*, because the name goes
through the same translation a browser's does and that translation carries a name
and never a path. Resolving it beneath the backups directory is the core's, and a
name climbing out of that directory reaches the core's own refusal rather than the
file it climbed to.

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
  words on the stream and `events::stepping::Stepping` puts a walk's steps there,
  and the context both say them through belongs to the run rather than to a job, so
  two actions running at once are narrated into one undifferentiated line of talk.
  A followed service's lines are the one narration that says whose they are, and
  they say the service rather than the job. What each piece of work *came to* is
  asked for by name at `/api/jobs/<name>`; what it is saying on the way there is
  anonymous.

## Related

- [dispatch.md](dispatch.md) — the one entry point all three surfaces go through
- [module-layout.md](module-layout.md) — where each surface's code lives
- [G1 interface tiers](https://github.com/lemonfiber/spec/blob/main/10-functional/features/g-ux/g1-interface-tiers.md) — the requirement and its four exceptions
- [web-api contract](https://github.com/lemonfiber/spec/blob/main/20-architecture/contracts/web-api.md) — the converse rule: the web exposes nothing the command line cannot do
