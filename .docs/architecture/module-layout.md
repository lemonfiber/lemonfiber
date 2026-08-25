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
│   ├── render/           what each outcome looks like
│   ├── acting/           what a dashboard keypress asks for and what becomes
│   │                     of it — every decision the terminal file must not hold
│   ├── examples/         emitters: a `print!` around one generated artefact
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
├── lemonfiber-fixtures/  lib — the fakes for those traits, reachable from both
│                              in-crate tests and `tests/`. Depends on ports only.
│
└── lemonfiber-manifest/  lib — stack.toml parse + validate
```

The `lemonfiber` package carries a library alongside its binary. The library holds
only the clap definitions and the renderers that turn what the workspace declares
into `reference/commands.md` and `reference/error-codes.md`; `main.rs` and everything
it reaches stay in the binary. The split exists because an artefact is written by a
program that is not this binary, and it has to read the same declarations rather than
a second description of them. `cargo run --example reference`, `--example codes` and
`--example contract` are those programs; each is a `print!` around one function, and
`just reference`, `just codes` and `just contract` redirect them to the file the tests
compare against.

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

## What the architecture test checks

`crates/lemonfiber/tests/architecture.rs`, run by `cargo test`:

| Test | Enforces |
|------|----------|
| `the_core_has_no_user_interface_dependency` | The core cannot render — no `ratatui`, `clap`, `axum` or friends in its manifest |
| `talking_to_the_outside_world_only_happens_in_adapters` | Each external crate appears in exactly one file |
| `only_the_platform_module_asks_which_operating_system_this_is` | No `target_os` outside `platform.rs` |
| `no_lint_is_suppressed_in_source` | No `#[allow(…)]` anywhere in `src/` |
| `no_requirement_identifier_appears_in_a_comment` | Spec identifiers stay in commits |
| `no_feature_requirement_identifier_appears_in_a_comment` | The same, for area identifiers with no fixed prefix |
| `no_source_file_outgrows_reading_in_one_sitting` | No file holds more than 550 production lines |
| `no_test_file_covers_more_than_one_seam` | One test file per seam, so a fake has one owner |
| `each_requirement_is_claimed_by_one_row` | Every requirement appears exactly once in the status table |
| `no_two_problems_answer_to_the_same_code` | An error code an operator searches for means one thing |
| `a_failure_is_reported_on_stderr_and_never_on_stdout` | A failure never lands in a piped stdout |

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
