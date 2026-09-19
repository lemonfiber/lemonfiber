# Module layout

Where things live across the workspace, and which rules a test enforces.

The crate boundaries and the module list are fixed in the spec's
[component-model](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md).
This page covers what that document deliberately leaves to this repo: the Rust
mechanics.

## The crates

```
crates/
├── lemonfiber/           bin + lib — the only crate that draws a terminal
│   ├── lib.rs            the command line, and the references generated from it
│   ├── cli.rs            clap definitions, non-interactive paths
│   ├── reference.rs      renders the command reference from those definitions
│   ├── codes.rs          renders the error-code reference from the crates' own
│   │                     declarations, read out of their source
│   ├── reaching.rs       which requests the dashboard reaches, published where a
│   │                     guard outside the binary can hold the parity table to it
│   ├── render/           what each outcome looks like
│   ├── acting/           what a dashboard keypress asks for and what becomes
│   │                     of it — a file per flow, each holding the list it
│   │                     decides over, and every other decision the terminal
│   │                     file must not hold
│   ├── examples/         emitters: one generated artefact each, printed or
│   │                     written where it is a set of files
│   └── tests/            the architecture tests, from the top of the graph
│
├── lemonfiber-core/      lib — all logic, no UI, no terminal
│   ├── app/              the one entry point: command in, outcome out
│   ├── model/            the values surfaces render, and serialise
│   ├── adapters/         the only code that talks to Docker, HTTP or processes
│   ├── platform.rs       the only cfg!(target_os)
│   └── …                 one directory per subsystem — doctor, seed, config, …
│
├── lemonfiber-api/       lib — the JSON endpoints answered on loopback, and the
│                              serving of the web app beside them. Chooses no
│                              address: the binary binds the socket.
│
├── lemonfiber-ports/     lib — the traits the outside world is reached through,
│                              and the vocabulary that crosses them. Depends on
│                              nothing of ours but the manifest.
│
├── lemonfiber-adapters/  lib — the implementations of those traits that reach the
│                              machine: the container runtime, the transport, the
│                              TLS connection, the disk. Depends on ports only, and
│                              sits above core rather than below it — the core is
│                              handed these, and cannot build one.
│
├── lemonfiber-fixtures/  lib — the fakes for those traits, reachable from both
│                              in-crate tests and `tests/`. Depends on ports only.
│
└── lemonfiber-manifest/  lib — stack.toml parse + validate
```

The `lemonfiber` package carries a library alongside its binary. The library holds
only the clap definitions and the renderers that turn what the workspace declares
into `reference/commands.md`, the pages under `reference/commands/`, and
`reference/error-codes.md`; `main.rs` and everything it reaches stay in the binary.
The split exists because an artefact is written by a program that is not this binary,
and it has to read the same declarations rather than a second description of them.
`cargo run --example reference`, `--example codes` and `--example contract` are those
programs, and `just reference`, `just codes` and `just contract` run them.

`--example codes` and `--example contract` are a `print!` around one function, which
the recipe redirects to the file the tests compare against. `--example reference` is
not, because the command reference is a set of files rather than one: an index at
`reference/commands.md` and a page per top-level command beside it, each carrying
everything declared beneath that command. The grouping is clap's own tree rather than
a table somebody maintains, so a command added gets a page and one removed takes its
page with it — which a redirect of stdout could not do, so the renderer writes the
directory itself and removes what it no longer has. The tests compare every page
against the declarations **and** the directory's contents against the set of pages,
because a page left behind for a command that is gone would otherwise match itself
for ever.

`--example surface` is the fourth and the one that is not merely a `print!`: it reads
the artefact it is about to replace before it writes, and refuses where the new one
drops a name or a type the committed one describes under an unchanged wire version.
That is the difference between the two contract artefacts. `web-api.contract.json`
says what the surfaces exchange now, so a comparison against it can only ever say
"regenerate it" — a removal and an addition are equally stale to it.
`web-api.surface.json` is names and types with every description stripped out, so it
moves only when the interface moves, and it is what a removed or retyped field is
caught against. `just surface` writes it through a temporary file, since a redirect
would truncate the very thing the program compares against.

`codes.rs` reads source text rather than values, because a code is a `const` beside
what raises it and there is no registry to enumerate. It reads it with a lexer that
tells code from a string from a comment, so the call it looks for is invisible where
it is merely quoted, and it reports a declaration it cannot account for rather than
leaving it out. The same reader answers the architecture test that no two problems
share a code, so what counts as a declaration is decided in one place.

`lemonfiber-core` re-exports the ports crate as `crate::ports`, so call sites read
`ports::Engine` whichever crate they are in. Why the boundary is a crate rather
than a module — and what the orphan rule decides about which types may cross —
is in [ports-and-adapters.md](ports-and-adapters.md).

The dependency arrow only ever points down that list. A port cannot reach the
logic above it, which is what makes the fixtures crate possible: a fake needs the
trait and nothing else, and if it needed `lemonfiber-core` it would be a
dev-dependency cycle.

## Files, not directories

`doctor.rs` + `doctor/` rather than `doctor/mod.rs`. Both work; the former means
a module's own documentation is not buried under a directory listing, and
`git log` on `doctor.rs` shows changes to the module rather than to a folder.

A file splits into a directory when it stops being one concern — or, failing
that, when it crosses 550 production lines, which is the mechanical floor the
architecture test puts under that judgement. The split goes at a seam the file
already has: the parent keeps the type and the surface, and each child takes one
question the type answers. Moved items widen to `pub(crate)` — a parent cannot
see a child's private items, and `mod child; use child::*;` compiles happily
while importing nothing at all.

## What the architecture tests check

One file per seam under `crates/lemonfiber/tests/`, all run by `cargo test`. The
file name is the question; the tests inside it are the ways of asking:

| File | Enforces |
|------|----------|
| `what_the_build_forbids.rs` | The core cannot render and cannot reach the network; the ports and fixtures crates depend on nothing of ours that would make them a cycle. Read from the manifests, where a dependency is actually enforced |
| `where_the_outside_world_is_reached.rs` | Each external crate appears in exactly one file, and no `target_os` outside `platform.rs` |
| `what_a_source_file_may_not_say.rs` | No `#[allow(…)]` anywhere in `src/`, and no spec or area identifier in a comment |
| `how_long_a_file_may_be.rs` | 550 production lines a shipped file, 1,200 a test file, and the test module declared where the counter stops |
| `the_one_way_out.rs` | Output leaves through `say.rs`, treated on the way; a failure lands on stderr; what a parser reads is never folded for a person |
| `each_requirement_is_claimed_once.rs` | Every requirement appears exactly once in the status table |
| `one_code_one_problem.rs` | An error code an operator searches for means one thing |
| `what_a_check_can_see.rs` | Every diagnostic check is handed something to ask, and says how long it disturbs the stack for |
| `a_latch_is_settled_once.rs` | Reading a value settled at startup never settles it |
| `nothing_shapes_this_machines_traffic.rs` | Nothing shipped reaches for a traffic shaper |
| `what_seeding_does_in_order.rs` | Every declared API kind is acted on, and the request service has an owner before anything is registered into it |

They read source text rather than the compiled crate. That is coarse and it is
enough — every rule above is about where a *name* is allowed to appear, and a
name that appears in a string but not in code is a false positive we would
rather have than the false negative.

It lives in the binary crate because that crate sits at the top of the graph and
can see every source file, and because a test crate inside `lemonfiber-core`
would be checking itself.

### The one it caught first

The identifier test failed on its own doc comment, which used a real identifier
as an example. The rule is stated in prose there now. Worth knowing before you
write the next one.

## Naming, and why it looks slightly off

Clippy's pedantic set includes `module_name_repetitions`, and suppressions are
not available. So the traits are named for what they are rather than for their
module:

| Reads | Rather than |
|-------|-------------|
| `ports::docker::Engine` | `docker::DockerApi` |
| `ports::process::Runner` | `runner::Runner` |
| `ports::service::Client` | `service::ServiceClient` |
| `ports::time::Clock` | `clock::Clock` |
| `adapters::process::Local` | `process::LocalRunner` |

The spec calls the seeding trait `ServiceClient`; here it is `service::Client`,
re-exported from `ports` so call sites read `ports::Client`. Same trait, and the
lint stays satisfied without an exception.

## Related

- [dispatch.md](dispatch.md) — the entry point every surface goes through
- [ports-and-adapters.md](ports-and-adapters.md) — the seam and how to fake it
- [error-model.md](error-model.md) — what every failure looks like
