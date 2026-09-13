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

## The seven that answer twice

Several commands here already give two answers: unconfirmed they say what they would
do, confirmed they do it. `reset`, `remove`, `quality upgrade`, `migrate`, `update`,
`restore` and `doctor --fix` are all shaped that way, and the unconfirmed answer
*is* the rehearsal — the same report, in the same words, filled in by the same code
path.

So a rehearsal takes the operator's go-ahead back on the way in
(`rehearsal::carried` → `unconfirmed`) rather than seven handlers each learning a
second way to say what they already say. A second way is a second thing to keep true,
and the one nobody exercises is the one that stops being true.

That function is deliberately *not* exhaustive: it is the mechanism, not the
decision. What a rehearsal means is decided in `asked`, which the compiler does check,
and a command that claims to report while its go-ahead is not taken back here fails
the gate against a real disk.

`doctor --fix` needed one thing beyond the withheld yes. It writes the fault store on every
run, before it decides whether to act, because the offer is built from how often a
fault has been seen and how often a fix left it standing. A rehearsal reads that store
and no longer adds to it: recording a sighting would move the counts, so the next real
run would decide differently because somebody had asked a question.

## What a rehearsal says

The same words the real run uses. `LifecycleReport` carries `rehearsed: bool` and the
same `command`, `plan` and `stack_edits` either way;
[`render/stack.rs`](../../crates/lemonfiber/src/render/stack.rs) prints `would run:`
and the exact argv above the report it would have printed anyway. A rehearsal that
spoke a vocabulary of its own would be teaching the operator that the rehearsal is
not the thing.

The same rule holds for the work a rehearsal has to do in order to have something to
say. Materialising the stack is one walk over the same files with the writing left
out (`materialise::would_materialise`), not a second reckoning of what is on disk —
two reckonings would drift, and the one nobody runs would be the one that is wrong.
