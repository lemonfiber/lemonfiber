# Dispatch

How a surface asks for something, in Rust.

The rule — every surface reaches behaviour through one entry point, and no
surface orchestrates the core directly — is in the spec's
[component-model](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md).
This is how it is built.

## The three types

```rust
pub enum Command { Version }              // what was asked for
pub struct Ctx { dry_run: bool, runner: Arc<dyn Runner> }   // everything else it needs
pub enum Outcome { Version(VersionReport) }                 // what came back

pub async fn dispatch(command: Command, ctx: &Ctx) -> Result<Outcome, Problem>;
```

`clap` parses into a `Command`. A keypress will build a `Command`. An HTTP route
will build a `Command`. None of them can do anything else, because there is
nothing else public to call.

## `Command` and `Outcome` are deliberately exhaustive

Neither is `#[non_exhaustive]`, which is unusual for a library type and is the
point. The surfaces ship in the same binary as the core, so adding a command
*should* stop the build until every surface has decided what to do with it. A
wildcard arm would let a new command render as nothing at all, which is the exact
failure the single entry point exists to prevent.

If the core ever ships to a consumer outside this workspace, revisit this.

## `dry_run` lives on the context

Not on each command, and not as a parallel code path. A rehearsal and a real run
differ in one field, so there is no second implementation to fall out of step
with the first — the golden tests that cover a rehearsal are covering the real
thing too.

```rust
let ctx = Ctx::new(Arc::new(Local)).rehearsing();
```

## Why `dispatch` is async when nothing in it blocks yet

Because the first command it carries out already reaches a port. `Command::Version`
asks the engine for its version through `Runner`, which is genuinely async, so
the signature is honest today rather than aspirational.

That was a deliberate choice of first command. A version report that only read
constants would have needed a fake `.await` to satisfy `clippy::unused_async`,
and a fake await is a lie that survives into every future reader's mental model.
Reaching a port instead means the whole spine — command, context, port, outcome,
envelope — is exercised by tests from the first commit.

## The spine, end to end

```rust
let ctx = Ctx::new(Arc::new(Local));
let outcome = dispatch(Command::Version, &ctx).await?;
println!("{}", serde_json::to_string(&outcome.envelope())?);
```

`Outcome::envelope` wraps it with `api_version` and a `kind`, so machine-readable
output is the same value a person sees rather than a second rendering of it.

`Outcome` implements `Serialize` by hand — it forwards to the payload rather than
tagging itself, because the envelope already carries `kind` and a serde tag would
put the discriminant in twice.

## Errors come back as values

`dispatch` returns `Result<Outcome, Problem>`, never a formatted string. The core
cannot print, so a surface receives the parts and decides how to show them —
colour and wrapping in the terminal, an object over HTTP. See
[error-model.md](error-model.md).

## Testing it

A fake `Runner` and no daemon:

```rust
struct Scripted(Result<Output, Failure>);

#[async_trait]
impl Runner for Scripted {
    async fn run(&self, _argv: &[String]) -> Result<Output, Failure> { … }
}

let ctx = Ctx::new(Arc::new(Scripted(Ok(spoke("v2.32.1")))));
```

Four cases are covered for `Version` alone: the engine answers, the engine is
absent, the engine runs and fails, the engine will not start. All four report the
version — an operator asking what is in play is usually doing it *because*
something is broken, so it must answer when the engine is down.

## Related

- [module-layout.md](module-layout.md) · [ports-and-adapters.md](ports-and-adapters.md)
- [error-model.md](error-model.md)
