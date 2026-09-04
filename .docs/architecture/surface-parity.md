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
| Terminal, writes | [`acting/offer.rs`](../../crates/lemonfiber/src/acting/offer.rs) · [`acting/errand.rs`](../../crates/lemonfiber/src/acting/errand.rs) · [`acting/lasting.rs`](../../crates/lemonfiber/src/acting/lasting.rs) · [`acting/quality.rs`](../../crates/lemonfiber/src/acting/quality.rs) · [`acting/surface.rs`](../../crates/lemonfiber/src/acting/surface.rs) · [`acting/disturbing.rs`](../../crates/lemonfiber/src/acting/disturbing.rs) | `OFFERED`, one key per action; the errands behind one more key; the two that keep going behind another; the three quality changes behind a fourth; the key that hands the terminal over; and the two widened reads, on no key at all because each is offered under the answer that named the gap |
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
| `up` | `up` | dashboard | Reachable in full from a browser, and on the terminal the whole stack is a choice too, because the command carries an empty list. Starting only some of a form's services was the gap, and the question it left the core — whether that is its own request, the way `Halt` is not `Down` — is answered yes: bringing a form up creates everything its closure holds and starting named services starts the ones named, which is the same pair `down` and `stop` are, and Compose spells both pairs differently. So the action forks the way `down` already forked, reaching `Command::Start` where services are named and `Command::Up` where none are. The command line still runs its own streamed start around it, because Compose narrates for minutes and that is not a value that arrives once — but both paths resolve what they are about through the same reading of the manifest, so neither can answer about a different set of services from the other. Several forms at once are named on the dashboard now: a row of its list is marked and the marked rows are what enter takes, so the list carries what a browser sends whole. The named services that fork are named there too, off a second list drawn beside the first one: the forms are taken, and then the services inside them, marked the same way. Which command that reaches is not the screen's to know — the names go through the same translation a browser's do, so the fork Compose spells two ways costs the terminal no second flow. The names come from the panel the box is drawn over rather than from a read of its own, taken from the manifest, so a service that has never started is on the list; naming none of them is going on with the forms, which is the request the screen always made. |
| `down` | `down` | dashboard | Reachable in full from a browser. The teardown is offered on both, and several forms are torn down at once on both, the dashboard's list being marked a row at a time. Stopping named services is offered on both now. It is `Command::Halt` rather than an argument to this one, and the dashboard reaches it the way it reaches the start beside it: the forms are taken off the list they always were, and the services inside them are marked off a second list built from the services the panels are already showing. The screen chooses between neither pair — every row goes through the same translation a browser's request goes through, and what comes back is what is carried. Letting anything still downloading finish first is a field on `Command::Down` now rather than a loop the command line ran around it, so a browser asks for it by saying so and is answered with a job's name, which is what a wait that can last an hour has to be answered with. What stayed on the command line is the question in front of it: it reports what stopping would interrupt, asks, and hands the answer over — and its companion `--yes` answers that prompt, which no machine-readable run is put, so it needs no web form. The wait and the named services cannot be asked for together, because what is in flight is a question about the download clients a form holds; both surfaces refuse the pair rather than dropping one of them. |
| `switch` | `switch` | dashboard | Reachable in full from all three. It refuses an empty `forms`, and `/api/forms` serves the names — which is what it was waiting on; the dashboard asks the same list of the core and offers the names it comes back with, so a stack declaring none is refused there in the same words. Several are named by marking them, and the empty list this command refuses is still not a thing the screen can send: the whole stack is offered as a row only where the translation carries one, and this action is not one of those. |
| `restart` | `restart` | dashboard | As `switch` on the web, and on the terminal it takes several forms the same way. The named services `--service` restarts are named there too: the gather the list was short of is the one the panels make every second, so the services inside the forms are taken off a list rather than typed. It keeps the forms it was given either way, which is what this command insists on and what naming no service goes on with. |
| `pull` | `pull` | dashboard | As `switch`, and reachable in full from all three for the same reasons: it refuses an empty `forms`, which `/api/forms` and the dashboard's own list both serve, and several are named by marking them. It is the one of the five that stops nothing, and it is asked about before it runs anyway, because it can spend an hour of somebody's connection — which is the more worth asking about the more forms it was given. |
| `ps` | `/api/status`, `/api/services` | dashboard | Reachable from all three. The dashboard and the endpoints are fed by the same gather. |
| `logs` | `/api/logs` | viewer | Reachable in full. The scrollback is a read that ends, and is answered with what it read; following does not end, so it is answered with a name for the work and the lines arrive on the stream the browser already holds — one endpoint for both, because the command line spells following as a flag on this request rather than as a request of its own. The stream carries a service's own lines now, which it did not: they go down as `log`, the event name every other envelope's kind becomes, so a browser that is not following never registers for one. Nothing is dropped at the source and a browser that reads more slowly than a service speaks is let go past the window the stream carries, which is the rule already in force for every other event. `--watch` is the terminal's own rendering of the same lines, not a separate request. |
| `config` | `config-set`, `/api/config` | dashboard | Reachable in full from a browser. Writing arrived before reading, so a browser could set a value it had no way to read back; `/api/config` answers both halves of the read the way the command line spells them — naming no setting shows every one, naming one reads that one. Credentials are withheld where the settings are read rather than where they are printed, so both the endpoint and the dashboard show what `config show` prints, and a guard refuses a screen that reads the file for itself. The terminal asks both halves too, and as the same two entries: one shows every setting in a box that moves through them, and the other opens the line a trace is typed on. Naming one is not a way past the withholding — the narrowing happens after the display path rather than instead of it, so a credential asked for by name comes back withheld exactly as the listing withholds it, and the screen reaches the file for neither half. An empty name is refused in the sentence a browser is refused with rather than read as having named none, which it was not before: it reached the core as a setting to look for, matched nothing, and came back as a listing of no settings — which reads as "there is no such setting" about a setting nobody named. Changing one is offered nowhere but the browser, like every other write. |
| `quality` | `quality-set`, `quality-reapply`, `quality-upgrade`, `/api/quality` | dashboard | Reachable in full from a browser. `/api/quality` serves the read the three writes were being made without: the preset in force, what each one means, and what it costs, which is the screen a browser is best at. The dashboard asks that read off its list of questions and offers all three writes behind a key of their own — not on the list of errands, because the agreement does not mean the same thing on these three as it does there. An errand that carries one answers, unconfirmed, with what it would do and changes nothing, which is what makes that run the account its question sits under; `quality-set` unconfirmed *records* the choice, and holds it only where this host would have to transcode the result in software, which is the one cost its agreement is for. A list whose rule is “unconfirmed says what it would do” cannot take an action for which that is false without the rule quietly becoming untrue for the ones it was written for. So what goes in front of each question is what that change really has to say, and the three do not have the same thing to say. Choosing is made off the four presets, each carrying what it means and roughly what an hour of it costs, because the run that would otherwise state the consequence is the run that records it — and a choice that comes back held rather than recorded puts the core's own caution on the screen with a second question under it, which is where the agreement goes on. Upgrading states what it would cost per media type and triggers nothing, which is the errands' own reading arriving where it fits. Re-asserting has nothing to say first and is not given a preamble for symmetry: it carries no agreement, and the core's report-only half of it is behind `--dry-run`, which is a property of a run rather than of a request, so no surface's action can ask for one. Which of the three carries an agreement is read from the same table the errands read it from. The media type a choice applies to is offered there now, as the step in front of the bars: the whole library, series, film, or music, each taken off a list rather than typed, because the four are compiled into this binary and a list already in hand costs nothing to show. What each of them may then be given is the translation's answer and not a second table kept at the screen — every resolution preset and every audio format is put to it for each media in turn, and only what comes back as a command is offered. That is the whole of why music shows three audio formats where the rest show four presets: `quality-set` given `music` reaches `Command::QualityMusic` and refuses a resolution, which is the fork `--for music` takes on a command line, learned here rather than written down again. A media type the table stopped accepting would stop being offered on that screen without anybody editing a list. |
| `doctor` | `/api/checks`, `/api/storage`, `diagnose`, `repair`, `undo`, `accept` | dashboard | Reachable in full from a browser. The diagnosis is served, and so is putting it right. `repair` is the offer and the consent in one action because they are one request read twice: unconfirmed it answers with what each repair would do and what else changes if it does, and confirmed it carries out what was agreed to — named by the offer it was read in, so an answer cannot be spent on an offer that has moved on since it was read. Confirmed while naming no offer is the standing consent `--yes` spells, which is a decision taken before there was anything to read rather than a way past being told. `undo` is its own action and carries no subject at all, the core deciding which repair was last and what reversing it takes; `accept` answers a warning. What was left was the diagnosis including the checks that disturb a running system, asked for on its own — those ran here only while repairing and while answering a warning — and the argument that kept it out is what settled it: a read that disturbed something would not be a read, so this is not one. `diagnose` is the same request at the door changes are asked for. It is the surface that forks and not the core: `Command::Doctor` already carries the widening as a field, exactly as the command line spells it as a flag, so unlike `Halt` beside `Down` there is no second command to add — the checks are the same checks, reporting a real verdict where an ordinary run reports them unverified. What differs is the method, and it has to: a `GET` that took the tunnel away to prove the killswitch comes back would be a `GET` that stopped somebody's downloads. The widening is required rather than defaulted, because a request without it would be `/api/checks` under a second name. The narrowing `--only` takes is offered beside it, because both disturbing checks name what to run in what they tell the operator — the release search says `--only services.releases --disruptive` outright — and a browser that could only ask for all of them would have to drop the tunnel to spend one indexer search. It is answered with a job's name, which is what a run bounded by how long it may hold the tunnel away has to be answered with. The dashboard shows storage and VPN facts the diagnosis also reads — how much room is left, whether imports link, where traffic leaves from and whether the client's traffic is inside the tunnel — and a fact is not a verdict. A pass, a warning and a check that could not be established all render as the same number in a panel, and no panel carries a remedy, which is the half of a diagnosis worth having. So the diagnosis is a question now, asked at `/api/checks` the way every other read this screen answers is, whole or narrowed to one family of checks or to one check by the id its finding carries — the last of which is what `/api/storage` is, asked for by name. The panels stay, because they are not a second rendering of the same run: one is a gather every second and the other is a diagnosis somebody asked for. The checks that disturb are offered **under that answer** rather than on a list or behind a key of their own, and the report is what makes that the right place: an ordinary run reports both of them unverified, and each of those findings says to run *that one*. So the widening is narrowed by whatever the reading was narrowed by, and an operator following either instruction does not take the tunnel away in order to spend one indexer search. It goes through the same `diagnose` a browser sends, carrying the same required word, since a read that disturbed something would not be a read here either — the fork is the surface's, and this surface's other door is the one every change goes through. Putting it right is reached too, behind a key of its own beside that reading. `repair` is asked for unconfirmed first, and that run *is* the offer — what each repair would do and what else changes if it does — so the account the question sits under is not a rehearsal of it. Those repairs are a list that takes several, marked one at a time, because the yes here is a selection rather than a bare agreement: this is the one action on any surface that shows the operator something and then acts on what they answered. The question sits under the very words the marked ones were offered in, and what the yes sends names the offer they were read in — so an answer cannot be spent on an offer that has moved on since, which the core checks against a fresh look before it carries anything out. `accept` sits beside it and is answered off the run that raised the warning: only something a run warns about can be accepted, so the warnings are asked for first and offered as a list to take one of, and a failure — which is not a choice — is not on that list. `--undo` is an errand instead of a third entry there. It reads no offer, answers no warning and names no subject at all, so its yes is the whole of the agreement, which is the errands' rule and not the other list's; it sits beside the archive put back, the narrower of the two reversals first. Two of the flags this request declares are spelled differently here rather than left out. `--yes` is the standing consent taken before there was an offer to read, and this screen has put the offer in front of the operator before it asks — so what it sends is the consent that names that offer, which is the same request carrying the check the standing form cannot make. `--fix-disruptive` asks for the one thing `--disruptive` asks for: the web carries a single `disruptive` for all three of its actions, and the command line spells it twice only because clap keys an argument by the field it sits on. Which half of a repair it belongs to is settled in the core — the half that *acts*, because an offer is what somebody reads before deciding and these checks prove themselves by disturbing, so an offer asked to include them is refused rather than widened. What is left of the flag there is a widening on the run that carries the repairs out, over checks that turn up no repair to offer, so it adds nothing to what was agreed to. The screen reaches that widening where it is the thing being asked for rather than a rider on something else: under the diagnosis, on the request `--disruptive` is spelled on. |
| `watch` | `watch` | dashboard | Reachable in full, and it is the command with no ending of its own: it holds until the data location is lost, which on a machine where the drive stays put is never. So it is answered with a job's name like every other long action, and the name is also how it is stopped — a browser has no interruption to send, so releasing the name is its Ctrl-C, and what the container engine was already asked to do goes on exactly as it does when a terminal is closed. A tab closing stops nothing, which is the useful case. What bounds an abandoned one is that asking what became of it renews it: a guard nobody has asked about across two sweeps of half an hour is let go and says so, so one started on a Friday does not outlive the day. Nothing else is leased, because everything else ends by itself. The dashboard offers it behind the key that opens the two that keep going, and it is the one thing on that screen offered an end — which is the same answer, asked of the same table: a terminal's interruption is what a released name stands in for, and on a screen in raw mode that interruption is a keypress rather than a signal, so escape lets the guard go and says that nothing was stopped with it. Leaving the screen leaves it guarding, and the line on the way out says it will not end by itself and that Ctrl-C ends it. It is chosen its forms off the same list an action's own subjects are chosen from and by the same movement, so one guard covers several forms at once; the whole stack is not among the choices at all, because the command refuses a guard with nothing to stop and the list is built by offering each subject to that refusal. |
| `trace` | `/api/trace`, `search` | dashboard | Served on the web, and it completes a screen that half-existed: `/api/requests` reports what the household asked for, and this follows one of them. What to follow is one query parameter — the command line takes it as words so it can be typed unquoted, and a query string carries the title whole. It is the one question the dashboard has to be given something before it can ask, so taking it opens a line to type the title on, and an empty one is refused in the sentence a browser is refused with. Narrowing to one season is offered there now, as a second entry on the same list: taking it opens the line the title is typed on and then a second line for the season, with what has already been given left on the screen above it. Typed rather than taken, for the reason a setting and a member are typed — the list a season would be picked off is the trace itself, which is the request the narrowing exists to avoid making. A season that is not a number is refused in the sentence a query string carrying one is refused with, rather than in one that screen wrote; the line takes digits and nothing else, so what is turned away is a keystroke rather than a request. "It is a second thing to type" was the argument for leaving it, and it argued for the second line rather than against the narrowing: that screen already had one. **Asking the indexers what they carry is offered on both, and it is a write.** A trace of something monitored that nothing has been grabbed for cannot say why from any record in the stack: the indexers carry nothing for it, or they carry releases the quality in force rejects, and those look identical from an \*arr's own history. Only a live search tells them apart, and that search is real — one request against the daily allowance the indexers hold the operator to. So the read stays a read and says which question is open, and the searching form is `search`, asked for at the door changes are asked for, exactly as the diagnosis that disturbs is. Required rather than defaulted there, for the same reason: a trace that asks the indexers nothing is the read already served, and two ways to ask one thing is the arrangement every read on this surface is kept out of. The name differs from the read's because one word answering at two doors is that same arrangement wearing a disguise. On the terminal it is offered where the diagnosis's widening is offered — under the answer that named the gap, since the plain trace says in as many words that the question is unanswered — and the show and the season the reading was narrowed by are what narrow the search, so nothing is typed twice and a browser or a screen asking where one season is does not spend the search on every season there is. |
| `household` | `/api/requests` | dashboard | Served on the web, and asked off the dashboard's own list of questions. Narrowing to one member is offered there now, as a second entry on the same list: taking it opens the line a trace is typed on, and what is typed fills the member a query string carries. Typed rather than taken off a list of the household, because the list it would be taken off is the answer the narrowing exists to avoid asking for — which is the difference between this and the two that are picked. An empty name is refused in the sentence a browser is refused with, which it also was not before: it matched nobody and came back as a household that has asked for nothing, the one reading this report is written to refuse. **It is a panel as well now.** Pending and failing requests are on the screen without being asked for, which is what `D4-R8` asks: a request waiting on a decision, or failed after one, is waiting on the operator, and an operator who has to think to ask is one who finds out when somebody comes to complain. The panel shows only those two states — a request being fetched or already here needs nobody, and listing it would push the ones that do off a panel this size — and the question behind it still carries every member and everything they asked for. Both are built from the one reading, so the screen and the question cannot report different requests. **What each member is held to is on the same line as what they asked for**, because the two are read together: the age limit reads with the certificates the operator's own media server names on either side of it rather than as a bare number, what happens to their unrated content is said whether or not it is on, and a member limited in what they may watch and not in what they may ask for is named in a finding rather than left to be spotted in a column. That last one costs a second service a read, and a service that will not answer costs the agreement rather than the household — an unread answer is not a disagreement. Under the list, where anybody carries a limit at all, the one sentence about what a limit here is not. |
| `front-door` | `/api/front-door` | dashboard | Reachable from all three, and new with the question it answers: which one address to hand somebody who lives here. The answer is derived in the core from what the stack declares — the request service where there is one, the library where there is not, and none at all where there is neither — so no surface picks a different door, and none of them can be talked into offering the page that links every service. It answers with the address as well as the service, built from what the machine says it is called at the moment of asking rather than from anything remembered, so a machine that has been renamed answers as it is now on every surface at once. It takes no arguments, so there is nothing a screen could leave out; the dashboard asks for it off the list one key opens, beside the household's own requests, **and shows the address in a panel of its own** without being asked — because the operator who needs it is not the one who thought to ask, and a screen already open should not send them looking for a command. |
| `walkthrough` | `walkthrough` | dashboard | Reachable in full, on the surface it was designed for: a first-time operator is likelier to be in a browser than in a shell. It is a job plus the stream — the report at the end is what the name is redeemed for, and each step goes down the stream the moment it is true, because a walk read back afterwards is a report and the operator would have learned what happened rather than watched it happen. The step goes down whole rather than as a sentence: the words are the core's own, and rendering them into a line for the browser would be a second copy of the walk's prose beside the one the terminal draws. Naming nothing is a request rather than an omission, and is offered as one. Reachable in full from the dashboard too, behind the same key: what to look for is typed on a line of its own and naming nothing there is the same request it is everywhere else, and the steps arrive in the box as they become true. They are drawn by the renderer a shell reaches for the same step, so there is one account of a walk on this surface and not two — and what it came to goes under the steps that were watched, which is the order a shell shows them in. It ends by itself, so the screen offers no end for it and leaving waits for it, as leaving waits for every other action. |
| `explain` | `/api/explain` | glossary | Reachable in full, and the one read that needs neither a stack nor a daemon: the words are a table compiled into the binary, so a browser meeting one in a failure can ask what it means while the thing that failed is still down. One endpoint over two commands, the way `forms` is: naming a word explains that one, naming none lists what there is to ask about — which a caller that has never met this vocabulary needs before it can name anything. Served rather than shipped; a second copy of the table in the web app would be a surface explaining a word its own way. The terminal offers it on `?`, over the words on the screen. |
| `outbound` | `/api/outbound` | dashboard | Reachable in full from all three, and the read with the least a surface could add of its own: it takes no arguments, because what it answers is read from this machine's settings and from the stack the manifest declares. An enumeration a caller could narrow would be one an operator could be shown half of, and half of *everything that leaves this machine* is the wrong half whichever half it is. The terminal asks it off the list one key opens, beside the other questions that take nothing. What the browser gets is the same list the shell prints and not a second copy of it: which requests exist, where each goes as this machine stands, exactly what travels, whether it is on, the setting that switches it off and what stops working once it is — plus the stack's own requests, headed as the services' rather than as lemonfiber's. |
| `stored` | `/api/stored` | dashboard | Reachable in full from all three, and the read a browser is least able to answer for itself: a page sees its own requests and nothing at all of the machine underneath it, so where lemonfiber's own files are, what each of them is for and which of them hold a credential is a thing only the process can say. It takes no arguments — the layout is the layout — and it needs neither a stack running nor a daemon reachable, because it is the layout this build carries against the directories this machine resolved. The terminal asks it off the list one key opens, beside the other questions that take nothing. |
| `invite` | `invite` | dashboard | Reachable in full from all three, and the request that most wanted to be terminal-only until it was written down here. What it produces is something the operator **passes on** — a name, one address, and a code to hold a phone against — so the command line is where it is most useful: read out, screenshotted, or photographed while standing beside the person joining. That is an argument for the terminal being good at it, not for the others being denied it, and the exception did not survive being argued. The screen offers it beside the other errands and opens a line to type a name on, which is the affordance a trace and a member already have; the name is **typed rather than taken** for the opposite reason a service is taken — the person being invited is not on the screen yet, which is the whole point of inviting them. Nothing is stateful on this side, so there is no ordering to keep between the surfaces: the account is made on the media server and read back from it, which is why an invitation outlives this program being closed and why a browser and a terminal cannot disagree about who has claimed one. The sweep of invitations nobody claimed rides along with the request rather than a clock, because nothing runs between commands, so every surface inherits it unchanged — with one exception every surface also inherits: the account being offered to is never among the ones taken back, because withdrawing means removing it and that is the account the offer is *for*. An invitation that has run out is therefore offered again on the account the person already had, keeping the identifier everything else in the stack knows them by, rather than deleted and rebuilt as somebody new wearing the same name. What the account is *for* is chosen here too, and on all three: which libraries the person may open and how far up the ratings they may go. They are one argument on this table rather than two, because they are one decision taken at one moment, and they reach only this request — a reissue takes a password off an account whose access somebody already chose and a removal takes the account away, so both refuse them by name. The command line spells them `--library`, once per library, and `--age-limit`, which takes the age the media server already holds a limit as. The screen asks them as the two answers after the name, and which shape each takes is the same rule every other pair here is decided by: the libraries are **typed**, because they are the media server's and reaching it is the one thing this screen does not do between a keypress and the frame after it — the same reading that has an archive typed and a service taken; and the age limit is **taken off a list**, because the steps it is offered as are a table compiled into this binary, which is a list already in hand. That list is built by asking the core for its steps and labelling each row in the core's own words, so a step added there is on this screen without anybody editing it — and the words on the row are the words a household read says the same limit back in, which is what keeps the surface that sets it and the surface that reports it from naming one setting two ways. Naming no library is saying nothing about libraries rather than asking for all of them, on every surface, and what is not said is not written — so offering somebody already here a second invitation cannot put their account back to open, and setting an age limit on one their household narrowed cannot widen it. **There is a third answer now, and it is asked only where the first two narrowed something**: what happens to content the media server has no rating for. A rating limit cannot decide about a thing that carries no rating, so the choice has to be made rather than defaulted silently — held back unless the operator says otherwise, with the cost of that said on the row rather than discovered later. The command line spells it `--unrated block` or `--unrated allow`; the request body carries the same two words, and a third word is refused by name rather than falling to whichever answer the shape happened to default to; the screen offers the two as a list, because they are a pair already in hand, and offers it **after** the limit and only where a library or a limit was actually chosen — an offer that narrows nothing writes no policy at all, so the question would be about a setting the run does not touch and a keypress the ordinary case does not owe. What the offer wrote comes back on the answer on all three, in the certificates this household's own media server names and with the one sentence saying that a limit here is a content filter and not a security boundary — which is the sentence a parent is likeliest to need and likeliest not to be told. |
| `reissue` | `reissue` | dashboard | Reachable in full from all three, and the third of the household errands — offering an account, letting its password be set again, and taking it away. What it does is put the account back to having **no password at all**, which is the state a fresh invitation leaves it in, so the person claims it again by setting the first one themselves at the media server. The operator never chooses a password and never sees one: the call carries a flag rather than a value, so there is nowhere to put one even in error — the same structural reason `D6-R2` holds on the invitation path. What every surface hands back is an **invitation**, not a report of its own, because after this the thing to send *is* an invitation: the same address, the same code, the same line about being asked to set a password. A second shape would be a second account of one message and the two would drift. No confirmation is asked for and none is offered: nothing is destroyed, nothing is listed first, and what ends is a password nobody here knew — a screen that made somebody agree to that would be asking them to weigh a loss they cannot see. The one account no surface will reset is the one this program signs in as, refused here rather than by the server for the same reason a removal refuses it. Every surface says the **deadline** and what happens at the end of it, in those words: a reset runs out on the same window an offer does, counted from the reset rather than from when the account was made, and what is withdrawn at the end is an account somebody has already watched on. That is a larger loss than an offer nobody took up, so “lapses” is not left to carry it — the message says the account is removed. |
| `remove` | `remove` | dashboard | Reachable in full from all three, and the counterpart to `invite` — the same errand read the other way, so it is offered beside it rather than filed with the destructive things at the end. The confirmation is inside the command, as a forget's is: unconfirmed it says exactly what goes — their watch history, which the media server offers no way to keep, and every request they made, which the request service destroys rather than reassigns — and touches neither service, so what a browser or a screen agrees to is **what it was shown** rather than a summary of it in other words. The name is typed rather than taken for the opposite reason a service is: the person is a row in `household`, not an object on this screen, and the surfaces disagree about what is on them. Nothing is stateful here either — both accounts are read back from the services that hold them — so a browser and a terminal cannot disagree about who is still in the house. What no surface offers is removing the account this program signs in as: it administers the media server, and the server refuses it in two different ways depending on why, neither of them a sentence anybody could act on. That refusal is made here instead, before the request, and reads the same on every surface. |
| `clients` | `/api/clients` | dashboard | Reachable in full from all three, and the read with the least of this machine in it: the table of what to watch on is the same answer everywhere, because the client landscape belongs to the platforms rather than to this stack. It asks nothing of the engine, and the one thing it does read from disk it reads best-effort — the quality preset on record, which with the platform decides whether playback here will be transcoded on the processor and struggle whatever app is installed. That caution is stated above the table, before a reader decides what to install, and it is absent far more often than present; where the preset cannot be read at all it falls back to the default, which warrants none. So this still answers on a machine with no stack set up at all — which is the case it is most wanted in, since somebody deciding what to tell the house is often doing it before anything is running. It takes no arguments: naming a device would let a surface show one row and call it the answer, and the row most worth reading is the one saying a device is poorly served and what to do instead. The terminal asks it off the list one key opens, beside the other questions that take nothing. What the browser gets is the same list the shell prints, in the same order — the order is the product's judgement about what somebody is likely to be holding, not a sort a surface may redo. |
| `forget` | `forget` | dashboard | Reachable in full from all three, and the confirmation is inside the command rather than in a screen: unconfirmed it lists exactly what `stored` lists and touches nothing, and only a second request carrying the agreement removes. That is deliberate — what is agreed to has to be *what was read*, and a removal that summarised the listing in different words would be a second account of the same thing. What it removes is two directories, which is every location the layout names; what it does not remove is named in the same answer, because somebody agreeing to this is entitled to have read that their library is not in the list before they agree rather than afterwards. On the terminal it is the last entry on the errands list, under the reset, so nobody lands on the most destructive thing here by pressing enter at a list they have only just opened. |
| `stuck` | `/api/stuck` | dashboard | Served on the web, which is where the dashboard's own "N stuck" figure lands: each entry is named the way `/api/trace` is asked, so the count leads somewhere. The terminal reaches it as a question now and follows one the same way the web's list does: taking the question asks that read, and the entries it comes back with are offered as a list to take one of, which asks `/api/trace` by that entry's own title. Taken rather than typed, because the title is on the screen already and retyping it is a spelling test. The panel stays and is not this: it renders the queue-health gather, which counts what has stopped and names the cause where several items share one, rather than naming each item the way a trace is asked for — two readings of one worry, and it was the second the screen could not reach. |
| `seed` | `seed` | dashboard | Reachable from all three. It is the first of the six errands the dashboard keeps behind one key, and the one with nothing to leave out: the command takes no arguments, so the question names what wiring the services to each other comes to and an explicit yes is the whole of what it needs. |
| `adopt` | `adopt` | dashboard | As above, and the pair it makes with `reset` is what the errands are ordered by. Keeping what an operator changed sits near the top of that list and throwing it away sits at the bottom of it, so nobody lands on the destructive one by pressing enter at a list they have only just opened. |
| `reset` | `reset` | dashboard | Reachable in full on both. It is the one write in this group that destroys work, so the terminal does not put the question until the stack has said what would be lost: the unconfirmed command goes first, the reverts it names are what the box shows, and the question sits under that account rather than over it. An effect somebody reads after agreeing to it is not one they agreed to, which is the reading every repair is already offered under. The agreement itself is the command's own field, so the screen carries no second idea of what confirming means. |
| `backup` | `backup` | dashboard | Reachable in full from a browser. It takes the one thing the command line takes — the single service to capture instead of the whole stack — and takes no path at all, because a capture goes into the backups directory lemonfiber chose, whoever asked for it. Refusing a stack that cannot be proven stopped is the command's own rule now rather than the command line's, so a browser cannot ask for the live-database capture a shell was never allowed either. That single service is offered on the terminal too, and it is picked rather than typed: the names are the gather the panels already hold, so a line to type one on — which would have been a name nothing checked before the capture ran — was never the choice. The list takes one rather than several, because an archive records one scope, and which of the two arguments an action fills is asked of the same table the action itself goes through. Where the container engine cannot be reached there is no service to choose between, so the capture is the whole stack it always was. |
| `support` | `support`, `/api/bundle/{name}` | dashboard, excepted | Reachable in full from a browser. Everything that decides what the bundle holds is offered: whether to produce one at all, how much of each service's log to take, whether media filenames survive, which settings are shown as they are, and the agreement that showing one takes. Where it goes is offered in the only terms a browser has — `--out` names a path on the host and a browser has none to name, so it is handed the file instead. A bundle asked for here is written with lemonfiber's own files under a name carrying the moment it was taken, and that name is what it is fetched back by. It is the better half of the trade rather than the poorer: a host path is useless in the case a browser is most used for, which is lemonfiber running on a machine nobody is sitting at. The name is resolved beneath the bundles directory by the core and never followed as a path; the fetch carries the token every other request carries; and the only destination that writes into that directory is the one this surface asks for — so what comes back is what this surface produced, under the agreement that produced it, and never a file the reads beside it withhold. The terminal offers three of the four that decide what is in one, beside the half that decides whether there is a file at all. How much of each service's log to take is typed on a line of its own, and an empty line is still the ordinary window. What becomes of media filenames is taken off a list of the two the command carries, opening on replaced. Both are said in the question above the yes, so what is agreed to is what is written. The third was invisible: the yes was spent on saying *write the file*, and the field the command reads for the operator's agreement went out false on every bundle that screen ever produced. It carries it now — an agreement that arrives only when it happens to matter is an agreement nothing carries. **Which settings are shown as they are is the second exception this table permits, and it is an exception rather than a gap.** A way past the withholding list on that screen would be a capability no other surface has, on the surface least likely to be sitting behind a login: a dashboard is opened on the machine, or over a session somebody already holds, and neither asks who is at the keyboard the way the token in front of every web request does. Two other ways out were put and both were declined. Building it there behind an agreement of the browser's own would be putting a credential in front of whoever is at the terminal and calling somebody else's door the evidence for it. Taking it off the browser too would remove a thing that works from the one surface whose door proves anything, to make a table symmetrical. So the reveal stays where it is, and the argument that kept it there is recorded here because it survived — the guard beside that screen's own list holds every bundle it can send to naming no setting at all, which is also what makes the agreement beside it safe to carry. Nothing leaves the machine on any of the three: the bytes go to the browser that asked, over the loopback connection it already holds. |
| `ui` | intrinsic | dashboard | **The one honest exception in this table, and it is the web's half of this row alone.** A surface cannot start itself: the request either reaches a server that is already serving, where it means nothing, or it means starting a second server, which is a different request — and it would make a running server hand out the per-run token for a new one. **Setting the password this request now sets is the same answer read from the other side.** A surface that could set the credential it is opened with is a surface one request could lock its operator out of, and the first one has to be set somewhere no credential is required yet — which on this request is the machine it runs on, and never the network. So this row is `intrinsic` in both halves of what it does rather than in one. It is not the exception on the terminal, where a key that starts the web surface and prints its address is meaningful. It is a key of its own rather than an entry on either list, because it is the one request on that screen that is not work sent to the stack: it reaches no action and no read, since no other surface has one to reach, and what it does is hand the terminal over. So it ends the screen rather than sharing it — the address, the warning that the connection is not encrypted and the token every request must carry are eleven lines somebody has to read and copy, which a box on a terminal in raw mode cannot offer and a torn-down alternate screen would take away. The five choices `--port`, `--no-browser`, `--assets`, `--set-password` and `--lan` make are made *under the question*, because after the yes there is no screen left to make them on: what the surface is about to be given is drawn on the same box the agreement is read on, one row each, and `y` starts it with exactly what those rows say. **The password is the one of the four whose value is not typed at that box, and it is the port's own argument read again.** Every line typed on this screen is drawn by this program into a box, and a box showing the credential in front of the most privileged surface in the product shows it to whoever is standing behind the reader — so the row carries whether one will be asked for and nothing else, and the asking happens on the ordinary terminal this screen has just given back, twice, with neither answer echoed. Two presses still take the surface at its defaults, which leave the password exactly as it stands and this surface on this machine. **How far it may be reached is the fifth row, and it is a choice between two arrangements rather than an address.** A screen naming an address would be a screen naming somewhere nobody argued for; naming a tier reaches the same policy the command line reaches, which refuses the network outright where no password is set — on the ordinary terminal this screen has just given back, in the same problem, naming both ways out. Two presses still take it at its defaults, and those defaults are the command line's own rather than a second set that agrees today. **The argument that kept the three out was that a port typed at a screen would be a number nothing had checked was free.** Nothing can check that and still be telling the truth a moment later — a port is free until something takes it — so the check the command line makes is the bind itself, in `taken`, which is inside the request rather than around it. A port typed here reaches that same bind and is refused by that same problem, which names the address and offers both ways out, on the ordinary terminal this screen has just given back. What is checked at the screen is the half that can be: that the word is a port at all, answered once beside the request for every surface that has to turn a word into one, rather than by the command line's own parser alone. **The browser is worth being able to refuse**, because the desktop asked to open one is the host's and not the reader's: this screen is most useful at the far end of a remote session, where opening one reaches a machine nobody is sitting at and the line saying a browser has been opened is false to the person reading it. **The directory is the one path this product will take from a caller, and the operator at this screen is not one** — the rule that hands a browser a name and never a path is about resolving somebody else's path with the server's authority, and this process was started from the operator's own shell, on the host, and resolves it with exactly the authority they already had. What is served out of it is read-only and is held inside it by `within::beneath`, exactly as it is for `--assets`. |
| `restore` | `restore`, `/api/backups` | dashboard | Reachable in full from a browser, and the confirmation is inside the command rather than in a screen: unconfirmed it verifies the archive and answers with what it holds, having touched nothing, and only a second request carrying the agreement overwrites — so a surface that skipped the listing would be asking for something else, not rendering the same thing differently. That is what makes the listing the terminal's consequence-before-the-question: what the archive would overwrite is on the screen, and the question is the line under it. What was missing was the step before naming one: the name is typed rather than pointed at, and nothing anywhere said what this machine had kept, so a browser could only ask for an archive whose name it already knew. `/api/backups` is that listing, and on the command line it is `lemonfiber restore` with nothing named — the fork `forms` and `explain` take on their own word, taken here for the reason they take it: a surface that has to name a thing cannot know the names in advance. The name still travels as a name and is resolved beneath the backups directory by the core, so one carrying a path, or climbing out of that directory, is refused by name rather than followed — which is the poorer picker `G1` records, not a request a browser cannot make. Accepting a re-point is offered there now, and it is offered where the operator can already see why it is being asked. The unconfirmed run that lists what the archive would overwrite is the same run that names the data root it was taken against, so where the two differ the question under that listing *is* the re-point — the consequence-before-the-question this row already had, carrying the one decision it was short of, rather than a second agreement given blind. Where they do not differ nothing more is asked and nothing more is sent, which is what that field means on the web too. |

| `space` | `space`, `/api/space` | dashboard | Reachable in full from all three, and the confirmation is inside the command rather than in a screen, exactly as a forget's is: unconfirmed it accounts for the disk and offers, and only a second request carrying the agreement removes. Which is why the web needs both halves named here — `/api/space` is the account, and the action of the same name is the answer to it, reaching one command that reads the same way twice. It takes one argument, and deliberately only one. There is no argument choosing *what* to reclaim, because that choice is not a caller's to make: what a confirmed run takes is the downloads nothing ever imported and the archive parts already unpacked beside their contents, and nothing else is ever in it. A torrent still seeding is named, sized, and left with the operator with what removing it does to a tracker's opinion of them stated beside it — a consequence outside this machine is not one a surface can weigh — and anything the operator has already asked to be left alone is not on offer at any level of fullness. A narrowing argument would be a way to ask for those, so there is none to ask with. On the terminal it is an errand rather than a question, under the forget, and its own unconfirmed run is what the box is drawn over: the account is read first and the yes sits under it. |

| `stop-seeding` | `stop-seeding` | dashboard | Reachable in full from all three, and the one request in this table whose yes is **not** a flag. It is the other half of `space`, kept apart from it because the spec forbids bundling a torrent's removal with generic cleanup: that account offers what costs nothing and names what costs something without touching it, and this takes one of the named ones, files and all. What it costs is stated first — where the download stands, the ratio it is still earning, what a private tracker does about an account that stops earning one, and what goes with it and what stays — and the run that states it prints a name for that reading. Answering is saying that name back, and there is nothing else to say: the command carries no `confirm`, so the argument list refuses one by name, and a caller who never read the consequence has nothing to send. An answer given for a reading that has since moved is refused rather than spent, because the name is built from what was read. The terminal reaches it through the errands list, where the name is carried rather than typed — the screen holds the question open in the process that asked it, which is exactly what a browser cannot do and why the name exists. |

## What the table adds up to

Of the thirty-six requests, thirty-five reach the web in full, zero reach it in
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

On the other side of the table thirty-five reach the terminal in full, zero reach
the terminal in part, one reaches the terminal but for an exception, and zero have
no terminal form — which is where three stood seven slices ago, nine before the one
ahead of that, and twenty before the one that began it. Every request the command
line accepts is reachable from a screen, and every one of them is reachable whole.

**Both columns are finished.** No row in this table says `partial` any more. What
that column lost, it lost an argument at a time — a screen that reaches a request but
not one of its arguments has not finished reaching it — and the last four went
together: the media a quality choice applies to, the season a trace is narrowed to,
the re-point a restore may need, and what a bundle holds. Two exceptions stand, one in
each column, and each of them was argued rather than left.

**The terminal has an exception of its own now, and it is spelled as one.** `support`
reaches every part of a bundle but the settings shown as they are, and that one is
never coming to this surface: a way past the withholding list there would be a
capability no other surface has, on the surface least likely to be sitting behind a
login. The row says `excepted` rather than `partial`, because the two are not the same
claim — a gap is short of something somebody is going to build, and an exception is
short of something nobody is. It cannot say `intrinsic` either: that word means nothing
is reachable and nothing ever will be, which of a request reached in all but one
argument is simply untrue. So this column is counted three ways where the web's is
counted two, and the figure this paragraph gives for it is read back off the rows like
every other.

`ui` was the last row whose gap was that a screen offered a request and none of
the choices it takes. The key that hands the terminal over started the surface at
its defaults, and the port, whether a browser is opened, and the directory the
interface is served from are chosen under its question now — before the yes,
because afterwards there is no screen left to choose on. The objection recorded
here against them was that a port typed at a screen is a number nothing had
checked was free; nothing can check that and still be true a moment later, so the
check is the bind the command line already makes inside the request, and what the
screen answers is whether the word is a port at all. They are offered on enter
rather than on a key of their own, this screen having spent enough letters
already.

Seven of those rows closed in two goes, and each time because several of them were
one gap wearing several names. `switch`, `pull` and `watch` were each short of
nothing but a list that could name several of the stack's forms, and the list names
several now. `up`, `down`, `restart` and `backup` were each short of the services
their command line spells with `--service`, and those four closed together: the
names were a gather the list did not have, and the gather is the one the panels
behind that list already make every second. It is taken from the manifest rather
than from what happens to be running, so a service that has never started is on it.

Picking rather than typing was not a preference. A typed service name is a name
nothing checked before the work ran, and the read that would have checked one
reaches the container engine — which is the one thing this screen does not do
between a keypress and the frame after it. Where that gather could not be filled
there is no service to narrow to, so no list is opened and each of the four is the
request it always was.

Two of those four reach a different command when services are named — `Command::Start`
beside `Command::Up` and `Command::Halt` beside `Command::Down`, the way Compose
spells both pairs — and the other two carry the services as an argument to the
command they already reach. That difference costs the screen nothing, because the
screen assembles no command: every row goes through the same translation a browser's
request goes through, and whatever comes back is what is carried. What does differ is
one name against a list of them, and that is the same table's answer as well — an
archive records one scope, so the capture's rows carry no mark and enter takes the one
under the cursor.

Four more closed at once for the same kind of reason: a screen that could show a
list and not name one thing in it. A setting and a member of the household are
named by typing, on the line a trace was already typed on. A form and one of the
items whose download has stopped are named by taking one off a listing the screen
asks for first — that question's own read given nothing, so no listing is written
down twice. Which of the four is which is not a matter of taste: a form and a stuck
item are already written down somewhere the screen can read them, where a setting
or a member could only be picked off the very answer the narrowing exists to avoid
asking for.

A row can also *gain* two arguments without any figure moving, which is what happened
when an invitation learned to say what the account is for. Which libraries somebody may
open and how far up the ratings they may go arrived on the command line, at the action
and on the screen in one go, so no row ever read `partial` for them — a request whose
arguments land on every surface at once costs this page a longer **Standing** cell and
no arithmetic at all.

A row can also lose an argument without any figure moving, and `quality` was the case
in point: it read `dashboard, partial` while the dashboard offered none of its three
writes, went on reading it once all three were offered because the media type a choice
applies to was left to a browser, and reads `dashboard` now that the media is the step
in front of the bars.

The last four rows closed together, and what they had in common was that each was
short of one argument and each argument had a shape the screen already had. A media
type and what a bundle does with media filenames are taken off a list, because both
are a fixed set compiled into the binary and a list already in hand costs nothing to
show. A season and a log window are typed, because neither has a list to be taken off
— a season's would be the trace the narrowing exists to avoid asking for, and a number
has none at all. A re-point is neither: it is an agreement, so it goes where every
agreement on that screen goes, under the account that called for it. And the agreement
a bundle takes was not missing so much as unspent — the yes had been going into *write
the file* while the field the command reads for consent went out false, which changed
nothing about the file precisely because that screen names no setting to show as it
is. Nothing caught it for that reason, and carrying it is what makes the one thing it
does not offer an exception rather than an accident.

`doctor` is the row that took three slices to close, and it is worth saying why the
figure only moved on the third. The first made the diagnosis a question rather than
two panels of facts; the second offered the checks that disturb a running system
under that answer; and neither moved a number, because the row was still short of
what to *do* about what it read. The third put that right — the offer read, the
repairs marked one at a time, the yes naming the offer they were read in, a warning
answered off the run that raised it, and the last repair put back from the list of
errands. Two flags were argued rather than built. `--yes` is a decision taken
before there was an offer to read, and a screen that has just shown the offer sends
the consent that names it instead, which carries the check the standing form cannot
make. `--fix-disruptive` is the same widening `--disruptive` asks for, spelled twice
on a command line for a reason that is clap's rather than the request's, and the core
has settled which half of a repair it belongs to: the half that acts, an offer asked
to include those checks being refused rather than widened. The screen reaches that
widening on the request it is about. Neither flag is an exception this table permits;
both are the same argument reaching the same command by the door this surface has.

These nine numbers are read back from the table above by the guard, because a
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

Six arguments were made and did not survive, which is worth recording so they
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

**A gap is not smaller for being one argument wide.** "It is a second thing to
type", "the whole library is what most people want", "the shell is right there" —
each was offered for a row that reached a request and not one of its arguments, and
each is a reason to build the argument rather than a reason to leave it. A season, a
media type, a re-point and what a bundle holds were the last four of them, and what
building them took from the argument is that none needed a shape this screen did not
already have: a line to type on, a list to take one off, and an account read before a
question. The one narrowing that really is not coming is recorded as an exception
instead, and the difference between the two claims is the whole of what `G1-R1` asks
this page to keep straight.

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

## The terminal acts on thirteen and asks about nine

It did neither. The dashboard read and the log viewer read; the wizard was the
only screen that changed anything, and it only runs on a machine that has not been
set up yet. The argument for leaving it that way was that the shell is right there —
an operator reading `sonarr: unhealthy` can type the restart. It is true and it
proves too much: by the same reasoning the web needs no actions either, since the
operator could open a terminal. The operator this surface exists for is the one on
the far end of a remote session, who is the least able of the three to reach
another surface to act on what this one has just told them.

The dashboard offers the five the screen already showed state for — starting,
stopping, switching, restarting and fetching — and answers the nine reads it showed
nothing of, or showed the raw material of: the versions in play, how this stack is
doing, the forms it declares, the settings, the quality in force, what the household
asked for, where the household begins, where one of those things got to, and what
has stopped on the way. Beside them are the seven writes that are not about what is
running at all: the wiring,
keeping an operator's edits, throwing them away, a capture, a bundle, an archive put
back and the last repair put back; the two that keep going once they are started;
the key that hands the terminal to the web surface; the three changes to the quality
the stack aims for, which sit apart from the errands because what an agreement means
on them is not what it means there; the two that answer a diagnosis — the repairs it
found put right, and a warning about a deliberate choice accepted — which sit apart
for the same kind of reason; and the diagnosis widened to the checks that disturb a
running system, which sits apart from all five lists because there is one of it and
because what it is put under is a report rather than a rehearsal. No request the
command line accepts has a terminal form of `none` any more.

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
done twice: twenty-nine requests will not fit on one row of a footer, let alone in
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

**And one consequence is read before it is agreed to with no rehearsal at all.** The
diagnosis that disturbs a running system is not a question — every question here goes
through the table of reads, and a read that disturbed something would not be a read.
It is not an errand either: that list's rule is that an unconfirmed run says what it
would do, and this action has no unconfirmed run, because the widening is required
and a diagnosis that disturbs nothing is the read already offered. Joining it there
would make the list's rule untrue for the six it was written for, which is the
argument the three quality writes already won. Nor is it worth a key, on a screen
that refused a letter per errand. What it is is the second half of an answer somebody
is already reading — an ordinary run reports both disturbing checks unverified and
each finding says to run that one — so it is offered under that answer, and the
account the question sits under is the report that named the gap. The narrowing comes
with it: the word that narrowed the reading is the word that narrows the widening, so
nothing is typed twice and no list of the families of checks is written down on this
screen at all.

**And one yes is a selection rather than an agreement.** Every errand that carries
an agreement answers, unconfirmed, with what it would do, and then a single yes
carries the whole of it out. A repair is not built that way. Unconfirmed it *is* the
offer — each repair with what it would do and what else changes if it does — and the
yes that follows is given to *some* of it, which is why the offer is a list that
takes several and the consent travels as the checks marked out of it. It is the one
action on any surface that shows the operator something and then acts on what they
answered, and the table of arguments says so: `repair` is the only name under
`TAKES_CONSENT`. A list whose rule is "the yes is the agreement" cannot hold an
action for which that is false without the rule quietly becoming untrue for the ones
it was written for, so it sits on a key of its own with the accept beside it — the
argument the three quality writes already won.

**And the agreement is bound to the offer it was read in.** The command line gets
that for nothing: the run that acts is the run that looked, in one process, so there
is no gap for the offer to move in. This screen sends two requests as a browser
does, so it sends the name the offer gave itself and lets the core recompute it from
a fresh look — a name that no longer stands is refused, having carried nothing out.
A screen that agreed to "whatever is offered now" would be a surface that quietly
re-scoped a repair between reading and agreeing, which is the one failure the whole
arrangement exists to make impossible.

**Answering a warning is picked, never typed.** Only something a run warns about can
be accepted, and the core refuses anything else — so the warnings are asked for
first and offered as a list to take one of, which means this screen cannot send an
accept that comes back refused. A failure is not on that list: it is not a choice,
so there is nothing about it to have weighed. That is the rule a form and a stuck
item are already named by, applied to the one write that has to name a check.

**Putting the last repair back is an errand.** It reads no offer, answers no warning
and names no subject at all — which repair was last, what reversing it takes and
which of those need a service to reach are the core's — so its yes is the whole of
the agreement, which is the errands' rule exactly. It sits beside the archive put
back, the narrower of the two reversals first, so nobody reaching for the one that
puts back a single repair lands on the one that puts back the whole configuration.

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
