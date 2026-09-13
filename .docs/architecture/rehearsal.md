# Rehearsal

What `--dry-run` means here, where the decision is taken, and why a handler cannot
quietly ignore it.

The requirement is the spec's
([F1-R2](https://github.com/lemonfiber/spec/blob/main/10-functional/features/f-extensibility/f1-customisation.md)):
every command supports a dry-run mode that prints the exact underlying invocation.
The component model
([component-model.md](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md))
adds the shape it must take — the *same* code path as the real run, never a parallel
one that can drift.

## The failure this exists to prevent

`--dry-run` is declared `global = true` in [`cli.rs`](../../crates/lemonfiber/src/cli.rs),
so every subcommand accepts it, and `Ctx::rehearsing()` sets a boolean. A boolean is
a thing a handler can decline to read, and declining to read it does not fail: the
handler performs the write and then fills in a report that says it was a rehearsal.
Sixteen state-changing handlers did exactly that.

That is worse than not offering the flag. An absent flag errors, and the operator
finds out before anything happens. An accepted one that does nothing carries out an
irreversible operation on somebody who explicitly asked for a rehearsal, and then
tells them it was one.

There is a second shape, and it is worse again: a handler that reads the flag late.
Every lifecycle command materialises the stack to disk before it can build the
Compose invocation to report, and the check against running Compose sat below that —
so a rehearsed `up` ran nothing and had already written the whole stack out and
rewritten the record of what it wrote. Half-rehearsed leaves the machine in a state
neither the operator nor the code expects.

## Where the decision is taken

[`app/rehearsal.rs`](../../crates/lemonfiber-core/src/app/rehearsal.rs) answers it,
once per command, in a match over `Command` that the compiler checks. Four answers:

| Answer | Means | What `dispatch` does |
|--------|-------|----------------------|
| `Reads` | The command changes nothing | Runs it. The rehearsal *is* the command. |
| `Reports` | The handler says what would happen and does none of it | Runs it. |
| `Cannot(why)` | The effect cannot be known without producing it | Refuses the flag, naming the reason. |
| `Untaught` | Changes things, not yet taught to report | Refuses the flag. |

`Cannot` and `Untaught` are separate on purpose. Both refuse, and an operator reading
the message wants to know which they have met: one will not change, and the other is
a row on a board somebody is working through. Given one code between them the
temporary becomes indistinguishable from the permanent, which is how a temporary
state becomes a permanent one.

Where the sixty-four arms stand: **thirty-nine report**, **twenty-two change
nothing** (twenty-four command shapes — two arms carry a pair each), **three refuse
for good**, and **none is untaught**. Nothing carries `Untaught` today. The verdict
stays because it is the right answer for a command added tomorrow that changes
something nobody has yet taught to report — the alternative is an author having to
choose between teaching it in the same afternoon and quietly reading the flag and
ignoring it, which is the failure this file exists to describe.

## Why forgetting is not possible

Three things, each catching what the other two cannot.

**A new command does not compile.** `asked()` matches every variant of `Command`.
Adding one without deciding what a rehearsal of it means is a build failure while the
command is being written, which is the only moment the answer is cheap.

**An untaught command refuses at runtime.** The verdict is read in
[`dispatch`](../../crates/lemonfiber-core/src/app.rs) *before* the handler is reached,
so a handler that has not been taught never gets the chance to act. The failure mode
is a refusal with a remedy, never a successful real run.

**A claim of `Reports` is checked against a real disk.**
[`a_rehearsal_changes_nothing.rs`](../../crates/lemonfiber/tests/a_rehearsal_changes_nothing.rs)
drives every subcommand the command line accepts, as a rehearsal, against a real
configuration home, stack directory and data root on a real filesystem, and hashes
the tree before and after. A rehearsal that wrote, changed or removed a byte fails
naming the path.

That last one is the half a fake cannot do. The writes that hid longest never went
through a port at all — the materialised stack, the record of what was materialised,
the setup progress file, the quality selection are written with `std::fs` through
`config::store`, deliberately (see [ports-and-adapters.md](ports-and-adapters.md)).
A filesystem fake is blind to every one of them, which is why no test in this
repository could see the lifecycle commands writing the stack during a rehearsal:
the core's own tests use `Source::External`, whose file list is empty, so the write
never happened in a test and always happened in production.

The seams that reach *off* this machine are recorders rather than the real thing, and
that is not a weakening of the same claim: a real host adapter would install a launch
agent into the home directory of whoever ran the suite. The assertion is still that
they were asked for nothing.

## The eight that answer twice

Several commands here already give two answers: unconfirmed they say what they would
do, confirmed they do it. `reset`, `remove`, `quality upgrade`, `migrate`, `update`,
`restore`, `doctor --fix` and `support --write` are all shaped that way, and the
unconfirmed answer *is* the rehearsal — the same report, in the same words, filled in
by the same code path.

So a rehearsal takes the operator's go-ahead back on the way in
(`rehearsal::carried` → `unconfirmed`) rather than eight handlers each learning a
second way to say what they already say. A second way is a second thing to keep true,
and the one nobody exercises is the one that stops being true.

That function is deliberately *not* exhaustive: it is the mechanism, not the
decision. What a rehearsal means is decided in `asked`, which the compiler does check,
and a command that claims to report while its go-ahead is not taken back here fails
the gate against a real disk.

`support --write` is the newest of them and needed one thing beyond the withheld yes.
The run that writes nothing said what the bundle would hold and not where it would
land, so a rehearsal answered half the question an operator asks at a shell. Both
halves now resolve the destination through one function (`support::landing`), and the
description carries it — which is the better command for it either way, since the
moment the path can still change what somebody does is before the file exists.

`doctor --fix` needed one thing beyond the withheld yes. It writes the fault store on every
run, before it decides whether to act, because the offer is built from how often a
fault has been seen and how often a fix left it standing. A rehearsal reads that store
and no longer adds to it: recording a sighting would move the counts, so the next real
run would decide differently because somebody had asked a question.

## The records a read keeps of having read

Three commands change nothing an operator asked about and still write something: a
note that they ran. `doctor --fix` keeps the fault store above; `update self` keeps
`updates.json` beside the settings, which is how it knows not to ask the release list
again today; and answering a setup question keeps the resumable progress file, which
is the whole of setup's state. Each of those is read as normal under a rehearsal and
written on no run that only says what it would do, for one reason said three times:
recording that this run happened moves what a later one decides, and a question that
changes the answer has been answered on the operator's behalf.

`update self` was refusing the flag it did not need to refuse, which is its own small
defect — a command that says no to `--dry-run` teaches an operator the flag is
unreliable everywhere. It replaces nothing; what it answers with is the exact command
for whichever tool owns the copy that is running, and that answer is the same either
way.

## The commands that wait, and the ones that cannot be asked twice

A watch has no ending of its own — it holds until the data location is lost — so a
rehearsal of it cannot be the command with its last step left out. It reports the
watch instead: the location it would hold, how often it would look, and the exact
invocation it would run the moment that location went. That argv comes from the same
prelude a real stop is built from (`engine::invocation`), never from a sentence
written beside it.

The terminal's first-run conversation (`lemonfiber setup`, with no sub-action) still
refuses the flag, and that refusal lives in the surface because the conversation does
not go through the dispatcher. It is the walkthrough's reason: what a rehearsal would
report is what the operator has not typed yet. The steps a surface drives one at a
time — where setup stands, an answer recorded, the apply — are values that arrive
once, and every one of those says what it would write and writes none of it.

`doctor --undo` is the other path that does not go through the dispatcher, and the
verdict for it is taken inside `repair::retracting` rather than above it. A surface
asked to read the flag can be written without reading it, which is the whole failure
this module exists to prevent.

## Seeding, and the rule it keeps

`seed` and `adopt` are one pass over one graph, and the report they owe is per
connection: the field, what the service holds now, and what would be pushed. The
survey that produces it was already written and already shared — the three-way
reconcile in [`seed/drift.rs`](../../crates/lemonfiber-core/src/seed/drift.rs) that
every driver reads a connection through — but the writes sat inside those same
drivers, immediately below the observation, with no gate between the two.

The gate is now there, at each write rather than above the pass, because the pass is
where the report comes from: a rehearsal stopped at the door would have nothing to
say, and one that surveyed separately would be a second opinion about what lemonfiber
intends — the one nobody runs is the one that goes wrong. Four of the connections
share one body (`wire_one`, behind root folders, download clients, applications and
request targets), and that is where one gate covers four: `wire_one` takes the state
to answer with rather than a flag saying to answer with one, because it holds two
futures and three labels and has never seen a value — only the caller knows what the
connection would be made *to*. A future is inert until it is polled, so returning
before polling the register future is the write not happening rather than the write
being skipped.

Two states exist only here. `WouldWire { yours, ours }` carries what the service holds
beside what a real run would leave it holding, the same pair `Conflicted` shows and
for the same reason — a report saying a connection would be made without saying what
it would be made *to* is a count, and a count is what an operator asking already has.
`WouldAdopt` carries nothing, because an adoption writes to no service at all: what
moves is lemonfiber's record of what it expects, and the value is not shown for the
reason `Unmanaged` does not show one.

**A rehearsed seed issues nothing but reads.** That rule is stricter than "registers
no connection" and it is stricter on purpose: a sign-in is a `POST` that opens a
session on somebody else's service, and a torrent client answers a password test the
same way. Both are state left behind by a run that promised to leave none, and
`a_rehearsal_changes_nothing.rs` holds exactly that line — any request that is not a
`GET` fails it.

Four things the rule costs, each reported as something this pass could not tell rather
than told wrong:

- whether the torrent password lemonfiber recorded is still the one in force, which is
  answered by signing in;
- what the household is told, and which \*arrs the request service hands a request to
  — both read as the owner, and the owner's session is a sign-in;
- whether a drifted download client still reaches anything, which the \*arr answers
  only by being asked to test it, and the test is a `POST`. The drift is still
  reported; what is left out is the claim that it broke something.

And the keys the stack's own services read out of the environment are *named* rather
than gathered. Two of them are read by being made — the media server mints its key
when it is asked for one, and the listening server has no account at all until this
pass makes one, with a password minted and recorded to go with it — so a question that
gathered them would have created the very things it promised only to describe. The
names come from the services present, which is knowable without asking anything, and
the names are the whole of what the operator is deciding about: the values are the one
thing a question must never make a second copy of.

## One deliberate exception, written down

`setup --answer --dry-run` still proves a credential against the live indexer or
provider. That is a request to somebody else's service made by a run that changes
nothing here, and it leaves an authentication attempt in their log.

It stays, and the reasoning is the trade it is: the proof *is* the answer being
reported — `WizardReport.proof` carries what the service said, and `validated` records
what a test established rather than what a caller asserted — so a rehearsal that
skipped it would report something the real run does not, and a rehearsal whose report
differs from the real one is the worse defect of the two. Recorded here rather than
left to be rediscovered, because a deliberate exception that is written down is a
decision and one that is not is a bug waiting for somebody else to find.

## What a rehearsal says

The same words the real run uses. `LifecycleReport` carries `rehearsed: bool` and the
same `command`, `plan` and `stack_edits` either way;
[`render/stack.rs`](../../crates/lemonfiber/src/render/stack.rs) prints `would run:`
and the exact argv above the report it would have printed anyway. A rehearsal that
spoke a vocabulary of its own would be teaching the operator that the rehearsal is
not the thing.

`rehearsed: bool` is the shape the rest took as well, and always for the same reason:
every field of the report means one thing in both tenses, so the flag moves the verb
and nothing else. A capture is settled before it is written — the room measured, the
manifest described, the destination derived, the surplus retention has no room for
worked out — so `BackupReport` carries the real destination and the real prune list
and says *would*. A reversal is judged whole before anything is touched, so
`UndoReversal` carries the changes that would go back and, apart from them, the ones
that only go back where the service that made them is answering.

Where a report had no field that could carry the answer, one was added rather than a
second report shape invented: a bundle now says where it would land, a watch carries
the vigil it would keep, and a rotation carries what it would replace, where that
value lives and what would still be owed afterwards — never a value, and never one
generated in order to describe it. `SeedReport` took the same flag, and it changes one
sentence: a pass that wired things is run again once the rest is ready, and a pass
that said what it would do is run for real.

## What a rehearsal must not produce

A rehearsal of `credentials --rotate` mints nothing. A replacement generated to
describe a rotation is a secret that exists because somebody asked a question, and it
would then have to be kept or thrown away — and one thrown away may be one the service
has already taken. So the report names the credential, the place its current value is
kept, and what each consumer would still need afterwards, and stops there.

The same rule holds for the work a rehearsal has to do in order to have something to
say. Materialising the stack is one walk over the same files with the writing left
out (`materialise::would_materialise`), not a second reckoning of what is on disk —
two reckonings would drift, and the one nobody runs would be the one that is wrong.

Seeding mints nothing for the same reason the rotation does not: the torrent client's
web UI password and the media server's admin account are the two values this product
generates rather than reads, and both report as `WouldWire` with no `ours` at all —
absent exactly where a real run would make the value up rather than read it. The keys
published into the environment are named and never shown, the way an adopted value is
never shown, and a rehearsed seed writes no baseline record: the baseline is the only
memory of what lemonfiber wrote, and one saved by a question would have the next real
run compare against a record of connections nobody made.
