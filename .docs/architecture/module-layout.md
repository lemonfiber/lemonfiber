# Module layout

Where things live inside `lemonfiber-core`, and which rules a test enforces.

The crate boundaries and the module list are fixed in the spec's
[component-model](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md).
This page covers what that document deliberately leaves to this repo: the Rust
mechanics.

## The tree

```
crates/lemonfiber-core/src/
├── lib.rs
├── app.rs           the one entry point — Command in, Outcome out
├── model.rs         the values surfaces render, and serialise
├── error.rs         Problem, Severity, State, Remedy, Diagnose
├── ports.rs         ├── docker.rs   trait Engine
│                    ├── process.rs  trait Runner
│                    ├── service.rs  trait Client
│                    └── time.rs     trait Clock
├── adapters.rs      ├── docker.rs   Daemon — the only bollard
│                    ├── process.rs  Local — the only tokio::process
│                    └── time.rs     System — the only SystemTime::now
├── platform.rs      the only cfg!(target_os)
├── stack.rs         ┐
├── docker.rs        │
├── config.rs        ├ boundary fixed, contents arrive with their milestone
├── doctor.rs        │
├── seed.rs          │
└── journal.rs       ┘
```

Modules with nothing in them yet are not placeholders. They carry the reasoning
for what will land there, so the decision is recorded once rather than
rediscovered when someone starts writing that subsystem.

## Files, not directories

`ports.rs` + `ports/` rather than `ports/mod.rs`. Both work; the former means a
module's own documentation is not buried under a directory listing, and
`git log` on `ports.rs` shows changes to the module rather than to a folder.

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

## Where the deferred modules pick up

| Module | Arrives with |
|--------|--------------|
| `stack` | The compose driver — argument construction, form closure, lifecycle |
| `docker` | Landed — interpreting what the engine reports, and the health gate |
| `config` | The setup wizard — the environment file and paths |
| `doctor` | Diagnostics — the check trait and findings |
| `seed` | Seeding — wiring, drift, and the first writes |
| `journal` | Seeding, since that is the first subsystem that changes anything |

## Related

- [dispatch.md](dispatch.md) — the entry point every surface goes through
- [ports-and-adapters.md](ports-and-adapters.md) — the seam and how to fake it
- [error-model.md](error-model.md) — what every failure looks like
