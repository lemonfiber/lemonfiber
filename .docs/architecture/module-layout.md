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
│   └── tests/            two binaries: `architecture/`, the checks that read the
│                         whole workspace from the top of the graph, and
│                         `integration/`, the binary driven from outside
│
├── lemonfiber-api/       lib — the JSON endpoints answered on loopback, the
│                              serving of the web app beside them, and the
│                              contract artefact describing what they write.
│                              Chooses no address: the binary binds the socket.
│
├── lemonfiber-adapters/  lib — the implementations of the ports' traits that
│                              reach the machine: the container runtime, the
│                              transport, the TLS connection, the disk. Depends on
│                              ports only, and sits above core rather than below
│                              it — the core is handed these, and cannot build one.
│
├── lemonfiber-core/      lib — all logic, no UI, no terminal
│   ├── app/              the one entry point: command in, outcome out
│   ├── model/            the values surfaces render, and serialise
│   ├── platform.rs       the only cfg!(target_os)
│   └── …                 one directory per subsystem — doctor, seed, config, …
│
├── lemonfiber-fixtures/  lib — the fakes for the ports' traits, and the scratch
│                              directories tests write into, reachable from both
│                              in-crate tests and `tests/`. Depends on ports only.
│
├── lemonfiber-testing/   lib — the context a test drives a command through,
│                              built from one of two starting points. The core's
│                              own tests compile the same file as a module, since
│                              depending on this crate would build the core twice.
│
├── lemonfiber-plugin/    lib — plugin.toml parse, and the vocabularies a plugin
│                              is written against. Depends on the manifest only.
│
├── lemonfiber-ports/     lib — the traits the outside world is reached through,
│                              and the vocabulary that crosses them. Depends on
│                              nothing of ours but the manifest and the error model.
│
├── lemonfiber-error/     lib — Problem and Code, the withholding a problem's
│                              detail passes through, retry wording, plurals.
│                              Depends on nothing of ours.
│
└── lemonfiber-manifest/  lib — stack.toml parse + validate
```

The `lemonfiber` package carries a library alongside its binary. The library holds
only the clap definitions and the renderers that turn what the workspace declares
into `reference/commands.md`, the pages under `reference/commands/`, and
`reference/error-codes.md`; `main.rs` and everything it reaches stay in the binary.
The split exists because an artefact is written by a program that is not this binary,
and it has to read the same declarations rather than a second description of them.
`cargo run --example reference` and `--example codes` are those programs, and
`just reference` and `just codes` run them. `--example contract` and
`--example surface` are the same shape in `lemonfiber-api`, for the web-API contract
the api crate serves, and `just contract` and `just surface` run them.

`--example codes` is a `print!` around one function; its recipe writes the output
beside the artefact and moves it over the committed file, so a failed run leaves the
file as it was. `--example contract` writes its artefact the same way itself.
`--example reference` writes a set of files rather than one: an index at
`reference/commands.md` and a page per top-level command beside it, each carrying
everything declared beneath that command. The grouping is clap's own tree rather than
a table somebody maintains, so a command added gets a page and one removed takes its
page with it — which a redirect of stdout could not do, so the renderer writes the
directory itself and removes what it no longer has. The tests compare every page
against the declarations **and** the directory's contents against the set of pages,
because a page left behind for a command that is gone would otherwise match itself
for ever.

`--example surface` reads the artefact it is about to replace before it writes, and
refuses where the new one drops a name or a type the committed one describes under
an unchanged wire version. That is the difference between the two contract
artefacts. `web-api.contract.json` says what the surfaces exchange now, so a
comparison against it can only ever say "regenerate it" — a removal and an addition
are equally stale to it. `web-api.surface.json` is names and types with every
description stripped out, so it moves only when the interface moves, and it is what
a removed or retyped field is caught against.

`codes.rs` renders the error-code reference from `lemonfiber_error::codes::every()`,
the registry every code is declared in.

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
that, when it crosses 550 lines, which is the mechanical floor the
architecture test puts under that judgement. The split goes at a seam the file
already has: the parent keeps the type and the surface, and each child takes one
question the type answers. Moved items widen to `pub(crate)` — a parent cannot
see a child's private items, and `mod child; use child::*;` compiles happily
while importing nothing at all.

## Tests

Each crate's integration tests are one binary, `tests/integration/main.rs`, with a
module per file: the shared fakes are compiled once and the suite links once. Unit
tests sit beside their source in a `tests.rs` of their own, and every file — source
or test — is held to a cap over all of its lines: 550 for source, 800 for tests.

## What the architecture tests check

One module per seam under `crates/lemonfiber/tests/architecture/`, all one binary.
The file name is the question; the tests inside it are the ways of asking:

| File | Enforces |
|------|----------|
| `what_the_build_forbids.rs` | The core cannot render and cannot reach the network; the ports and fixtures crates depend on nothing of ours that would make them a cycle. Read from the manifests, where a dependency is actually enforced |
| `where_the_outside_world_is_reached.rs` | Each external crate appears in exactly one file, and no `target_os` outside `platform.rs` |
| `what_a_source_file_may_not_say.rs` | No `#[allow(…)]` anywhere in `src/`, and no spec or area identifier in a comment |
| `how_long_a_file_may_be.rs` | 550 lines a source file and 800 a test file, counted over the whole file, and a source file's tests kept beside it rather than inline |
| `the_one_way_out.rs` | Output leaves through `say.rs`, treated on the way; a failure lands on stderr; what a parser reads is never folded for a person |
| `each_requirement_is_claimed_once.rs` | Every requirement appears exactly once in the status table |
| `what_a_check_can_see.rs` | Every diagnostic check is handed something to ask, and says how long it disturbs the stack for |
| `a_latch_is_settled_once.rs` | Reading a value settled at startup never settles it |
| `nothing_shapes_this_machines_traffic.rs` | Nothing shipped reaches for a traffic shaper |
| `what_seeding_does_in_order.rs` | Every declared API kind is acted on, and the request service has an owner before anything is registered into it |

The workspace is crawled once and each file parsed with `syn`, so a rule about
code reads the syntax tree and a name in a comment or a string is never mistaken
for a call. The checks about comments read them with rustc's own lexer.
It lives in the binary crate because that crate sits at the top of the graph and
can see every source file, and because a test crate inside `lemonfiber-core`
would be checking itself.

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
