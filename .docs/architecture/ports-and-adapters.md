# Ports and adapters

The seam between logic and the outside world, and how to fake it.

Why the seam exists is in the spec's
[component-model](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md).
This is what it is made of.

## The four ports

| Port | Trait | Reaches |
|------|-------|---------|
| `ports::docker` | `Engine` | The Docker Engine API — state, stats, logs, exec |
| `ports::filesystem` | `FileSystem` | The data root — resolving it, and proving it can hardlink |
| `ports::process` | `Runner` | Spawned programs, which is how Compose is driven |
| `ports::service` | `Client` | The services' own HTTP APIs, for seeding |
| `ports::time` | `Clock` | The wall clock |

Each is `Send + Sync`, because a background poller owns one and the render loop
must never wait on it.

## `async_trait`, not native async fn

All four I/O ports use `#[async_trait]`. Native `async fn` in traits is stable,
but the resulting trait is not object-safe, and every one of these is held as
`Arc<dyn …>`:

- `Ctx` holds `Arc<dyn Runner>`, so a fake substitutes without a generic
  parameter threading through every call site.
- Doctor checks are values in a collection, which requires `dyn`.
- Service clients are selected at runtime by the manifest's `api.kind`, which is
  the entire reason that field exists — a compile-time type could not do it.

The boxing cost is one allocation per call against operations that spawn a
process or cross a socket.

`Clock` needs no attribute: `now` is synchronous.

## Streams are channel receivers

```rust
async fn logs(&self, project: &str) -> Result<Receiver<LogLine>, Failure>;
async fn stats(&self, project: &str) -> Result<Receiver<(String, Stats)>, Failure>;
```

Not `impl Stream`. The producer owns its data and sends owned snapshots, which is
what the render loop requires — nothing is shared with a frame, and a slow
producer delays its own panel and nothing else. A `Receiver` says that in the
type; a `Stream` would leave it to convention.

It also keeps `futures` out of the dependency graph.

## There is no filesystem port

Deliberately. Paths are injected and tests run against a temporary directory.

Abstracting the filesystem would buy fake confidence in the one place it matters
most: the storage probe has to create a file, hard-link it, and compare inode
link counts on the operator's *actual* volume. A faked filesystem would report
that hard-linking works on a volume where it does not, which is precisely the
silent degradation the probe exists to catch.

## What is implemented, and what is not

| Adapter | State |
|---------|-------|
| `adapters::process::Local` | Real. `tokio::process`, four tests. |
| `adapters::time::System` | Real. |
| `adapters::docker::Daemon` | Real. `bollard`; see [engine-api.md](engine-api.md). |
| `adapters::filesystem::Disk` | Real. Standard-library I/O; `sysinfo` for the filesystem type. |
| `adapters::http` | Not yet — arrives with seeding, on `reqwest`. |

The port for the last one is defined, so the logic above it can be written and
tested first. That ordering is the point of the seam: nothing waits on an
adapter.

The architecture test reserves their filenames — `bollard` is permitted only in
`adapters/docker.rs` and `reqwest` only in `adapters/http.rs`, the latter before
the file exists. A first attempt to reach the network from somewhere else fails
the build rather than being noticed in review, or not.

It is coarse enough to catch the *name* rather than the use: a prose mention of
`bollard` in a comment outside that file fails too. Reword the comment; do not
widen the rule.

## The adapter a trait fake cannot test

`Daemon` is the exception to everything above. Faking `Engine` to test it would
put the fake below the code under test and prove only that the fake works, so
what gets replaced is the daemon rather than the trait — a socket speaking the
Engine API, in `tests/engine.rs`. [engine-api.md](engine-api.md) covers it.

The rule this suggests generally: fake the port when the question is what
lemonfiber does with an answer, and fake the far side when the question is
whether the protocol is spoken correctly.

## Writing a fake

Implement the trait. There is no mocking framework and there does not need to be:

```rust
struct Scripted(Result<Output, Failure>);

#[async_trait]
impl Runner for Scripted {
    async fn run(&self, _argv: &[String]) -> Result<Output, Failure> {
        // `Failure` is not Clone — thiserror types rarely are — so rebuild it.
        match &self.0 { … }
    }
}
```

`Failure` deliberately does not derive `Clone`. Errors are moved, not copied, and
a fake rebuilding one per call is a small price for not putting `Clone` on a type
that will later hold a socket error.

## Where an adapter's error becomes an operator's problem

Adapters return their port's typed `Failure`. Turning that into something an
operator can act on happens through `Diagnose`, at the point where the failure is
handled rather than where it is raised — the same missing binary is a blocking
error during setup and an absent version field in a version report.

See [error-model.md](error-model.md).

## Related

- [module-layout.md](module-layout.md) · [dispatch.md](dispatch.md)
