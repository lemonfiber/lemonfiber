# Dispatch

How a surface asks for something, in Rust.

The rule — every surface reaches behaviour through one entry point, and no
surface orchestrates the core directly — is in the spec's
[component-model](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md).
This is how it is built.

## The three types

A context is the seams a run reaches the world through, bundled as `Seams`, plus
what the operator chose and how the run behaves. A command reaches a port as
`ctx.seams.http`; a context's builders are named `with_*` for a seam or an observer
replaced, and a verb for a way of running — `rehearsing`, `forcing`,
`recording_at`.

```rust
pub enum Command { Version, /* … */ }                             // what was asked for
pub struct Ctx { pub dry_run: bool, pub seams: Seams, /* … */ }      // everything else it needs
pub enum Outcome { Version(VersionReport), /* … */ }              // what came back

pub async fn dispatch(command: Command, ctx: &Ctx) -> Result<Outcome, Box<Problem>>;
```

`clap` parses into a `Command`. A keypress builds a `Command`. An HTTP route
builds a `Command`. None of them can do anything else, because there is
nothing else public to call.

## `Command` and `Outcome` are deliberately exhaustive

Neither is `#[non_exhaustive]`, which is unusual for a library type and is the
point. The surfaces ship in the same binary as the core, so adding a command
*should* stop the build until every surface has decided what to do with it. A
wildcard arm would let a new command render as nothing at all, which is the exact
failure the single entry point exists to prevent.

## The table

`dispatch` routes each `Command` to one handler, and every row reads the same way:
the handler returns its own report, and the row names the `Outcome` variant it
becomes.

```rust
Command::Space { confirm } => space::space(ctx, confirm).await.map(Outcome::Space),
```

A handler returns `Outcome` itself only where it answers with more than one
variant — a migration, the wiring.

A handler is named for the command it answers: `Command::Backup` reaches
`backup::backup`, and `Command::ConfigSet` reaches `configuring::set`. Where
several commands share one handler it is named for what they share —
`engine::lifecycle` answers up, start, stop, restart and pull — and a read is
named for what it reads: `engine::status`, `stored::listing`,
`self_update::standing`. A command whose row would need more than one line has a
helper beside the table, named for what it does there.

## `dry_run` lives on the context

Not on each command, and not as a parallel code path. A rehearsal and a real run
differ in one field, so there is no second implementation to fall out of step
with the first — the golden tests that cover a rehearsal are covering the real
thing too.

```rust
let ctx = Ctx::new(seams, stack, settings, environment).rehearsing();
```

## Why `dispatch` is async

Because the commands it carries out reach ports, and the ports are async:
`Command::Version` alone asks the engine for its version through `Runner`.

## The spine, end to end

```rust
let ctx = Ctx::new(seams, stack, settings, environment);
let outcome = dispatch(Command::Version, &ctx).await?;
println!("{}", serde_json::to_string(&outcome.envelope())?);
```

`Outcome::envelope` wraps it with `api_version` and a `kind`, so machine-readable
output is the same value a person sees rather than a second rendering of it.

`Outcome` implements `Serialize` by hand — it forwards to the payload rather than
tagging itself, because the envelope already carries `kind` and a serde tag would
put the discriminant in twice.

## Lifecycle: one function, every compose command

`up`, `pull`, `start`, `halt` and `restart` reach the same `lifecycle` function
with a different `Action`; `down` reaches it through `teardown`, which first waits
for the downloads a stop would interrupt when it was asked to, and a rehearsal never
waits. Resolve the manifest, resolve the forms, materialise the stack, build the
command, run it — and a rehearsal returns *after* the build and before the run, so
what it reports is the command that would run rather than an approximation of it.

A preview is that pipeline stopped after its second step: resolve the manifest, resolve
the forms, answer. It shares those two steps with the lifecycle path rather than repeating
them, so a preview and the command it precedes cannot disagree about which services a form
holds or why one was left out.

`ps` and `logs` are deliberately **not** here. They are reads, and reads go
through the Engine API rather than through Compose — polling a subprocess once a
second across twenty services would be both wasteful and visibly jittery.
`Action` therefore covers only `Up`, `Start`, `Down`, `Stop`, `Remove`, `Restart`,
`Pull` and `Config`, which is exactly the split the architecture draws.

## Errors come back as values

`dispatch` returns `Result<Outcome, Box<Problem>>`, never a formatted string. The core
cannot print, so a surface receives the parts and decides how to show them —
colour and wrapping in the terminal, an object over HTTP. See
[error-model.md](error-model.md).

## Testing it

A fake `Runner` and no daemon, through the builder in `lemonfiber-testing`, which
settles everything a test does not name:

```rust
pub struct Scripted(pub Result<Output, Failure>);

#[async_trait]
impl Runner for Scripted {
    async fn run(&self, _argv: &[String]) -> Result<Output, Failure> { … }
}

let ctx = a_context()
    .runner(Arc::new(Scripted(Ok(spoke("v2.32.1\n")))))
    .engine(Arc::new(Reporting::default()))
    .build();
```

Four cases are covered for `Version` alone: the engine answers, the engine is
absent, the engine runs and fails, the engine will not start. All four report the
version — an operator asking what is in play is usually doing it *because*
something is broken, so it must answer when the engine is down.

## Related

- [module-layout.md](module-layout.md) · [ports-and-adapters.md](ports-and-adapters.md)
- [error-model.md](error-model.md)
- [surface-parity.md](surface-parity.md) — which surfaces reach which of those commands, and what is missing
