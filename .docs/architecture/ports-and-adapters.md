# Ports and adapters

The seam between logic and the outside world, and how to fake it.

Why the seam exists is in the spec's
[component-model](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md).
This is what it is made of.

## The boundary is a crate

`lemonfiber-ports` holds the traits and the vocabulary that crosses them, and
nothing else — it depends on `lemonfiber-manifest` and no other crate of ours.
`lemonfiber-core` re-exports it as `crate::ports`, so call sites are unchanged.

A crate rather than a module, for two reasons that turned out to be one:

- The architecture says the boundary is the stable part and the logic above it is
  not. A crate makes that something Cargo enforces rather than a sentence here:
  a port cannot reach up into the logic, because the dependency does not exist.
- The fakes implement these traits. A crate's `#[cfg(test)]` modules and its
  `tests/` directory are separate compilation units, so a fake defined in either
  is invisible to the other — which is how the same port came to be faked twice
  and the filesystem four times. A fixtures crate fixes that, and it can only
  exist if the traits live somewhere `lemonfiber-core` is not: depending on core
  would be a dev-dependency cycle, and Cargo would build core twice, leaving the
  fake implementing a trait belonging to neither copy the test uses.

What moved is decided by the orphan rule rather than by taste: a type belongs to
the boundary only if *all* its inherent behaviour can come with it. `Manifest`
has behaviour, so the archive port that speaks it stayed in core; `Digest` is
assembled from core's own models, so the notification port stayed too. The test
is mechanical — if an `impl` block would have to be left behind, the type is not
vocabulary, it is logic.

## The seams

Held as `Arc<dyn …>` and faked in every test that does not want the real thing:

| Port | Trait | Reaches |
|------|-------|---------|
| `ports::docker` | `Engine` | The Docker Engine API — state, stats, logs, exec |
| `ports::filesystem` | `FileSystem` | The data root — resolving it, and proving it can hardlink |
| `ports::filesystem` | `Volume` | Whether the data root is still present, and still the same volume |
| `ports::http` | `Http` | Any HTTP request, which every service client is built on |
| `ports::narration` | `Narrator` | Where a long wait says what it is waiting for, for a surface to render |
| `ports::network` | `Site` | What this machine calls itself, which is the name another device on the network asks for it by |
| `ports::occupancy` | `Occupancy` | What a directory tree actually holds, file by file, with the identity that says which of them are one file |
| `ports::nntp` | `Nntp` | A Usenet provider, dialled directly to prove the credential works |
| `ports::process` | `Runner` | Spawned programs, which is how Compose is driven |
| `ports::random` | `Random` | The entropy a minted credential is drawn from |
| `ports::time` | `Clock` | The wall clock |

Each is `Send + Sync`, because a background poller owns one and the render loop
must never wait on it.

`ports::narration` is the only one that carries words *out* rather than reaching
something in. It is a port for the same reason the others are: the wait it serves
is minutes long and lives in the core, which has no terminal — so it says what it
is waiting for, and the command line prints those words under the command while
the web surface says them on the stream a browser holds open. A run whose surface
is not listening holds `Silent`, so there is no second code path for the case
where nobody is.

`ports::service` is the exception to the one-trait-per-module shape: a service is
not one capability but several, and which it has depends on what it is. `Client`
is what every service answers; `MediaServer`, `Requests`, `Indexers`,
`UsenetAccounts`, `Transfers`, `Queues`, `Library`, `Pipeline`, `Catalogue`,
`AppSync`, `Maintenance`, `Importing`, `QualityReleases` and `MusicQuality` are
asked only of the services that have them. Splitting them means a fake answers
the one question a test is about rather than standing in for a whole service.

`ports::error`, `ports::media`, `ports::trace` and `ports::withheld` define no
seam at all. They are the vocabulary that crosses one — a problem an adapter
reports, the stage an item has reached, a value deliberately not shown. A port
that could not name what it returns would push the naming into every caller.

## `async_trait`, not native async fn

Every I/O port uses `#[async_trait]`. Native `async fn` in traits is stable,
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

The streaming ports avoid `Stream` for the reasons above, not to keep `futures`
out of the graph: seeding and `doctor` take `futures-util`'s `join_all` to run
each service's independent work at once, so a run's time tracks the slowest
service rather than their sum. Only the combinator is taken — no executor — and
it is already in the tree via reqwest, so it adds no crate.

## The filesystem port is narrow, and the probe stays real

There is a `FileSystem` port, but a deliberately narrow one, and the concern that
made the early design resist a filesystem port still shapes it. The storage probe
has to create a file, hard-link it, and compare inode link counts on the
operator's *actual* volume; a faked filesystem would report that hard-linking
works on a volume where it does not — precisely the silent degradation the probe
exists to catch. So the probe's real work runs through the `Disk` adapter against
the real volume. The port exists so the logic *around* that raw I/O — resolving
the data root, deciding what a probe result means, reading a service's
configuration — is settled without touching a disk and driven against a fake,
while the one measurement that must be real stays real.

`ports::filesystem::FileSystem` covers reading, writing, canonicalising and
ownership; `ports::filesystem::Volume` covers whether the data root is still
present and still the same volume.

The boundary between the port and raw `std::fs` is deliberate: the port stands in
for what a *service* owns, where a fake must be able to answer for it; lemonfiber's
own small records — the config store (`.env`), the drift baseline, the change
journal, setup progress — are written with `std::fs` and exercised against a real
temporary directory rather than through the port, since there is nothing a fake
would add over the real thing and the secret-file mode handling has to be real.

## What is implemented, and what is not

| Adapter | State |
|---------|-------|
| `adapters::process::Local` | Real. `tokio::process`, four tests. |
| `adapters::time::System` | Real. |
| `adapters::docker::Daemon` | Real. `bollard`; see [engine-api.md](engine-api.md). |
| `adapters::filesystem::Disk` | Real. Standard-library I/O; `sysinfo` for the filesystem type. Implements `Volume`, `Eraser` and `Occupancy` too. |
| `adapters::http::Web` | Real. `reqwest` + `rustls`, with connect and request timeouts and a host-scoped cookie store for session-auth services. |
| `adapters::network::Here` | Real. `hostname` through the process port, so there stays one place in this workspace that spawns anything. |
| `nntp` (binary crate) | Real. `tokio-rustls` for a TLS-wrapped NNTP dial; lives in the binary crate, not core. |
| `archive` (binary crate) | Real. `flate2` + `tar` for backup/restore; lives in the binary crate, so core carries no archive dependency. |

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

First, do not. `lemonfiber-fixtures` holds one fake per port, and it is a
dependency of both `lemonfiber-core`'s own tests and its `tests/` directory —
which is the whole reason it is a crate. Reach for the shared one; a bespoke fake
for a wide trait leaves every method the test does not call uncovered, and the
coverage gate counts them.

Where a genuinely new port needs one, implement the trait. There is no mocking
framework and there does not need to be:

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

Fakes are scripted, not subclassed: a test says what a service answers, it does
not write another service. A request nothing scripted is unreachable rather than
a helpful default — a test that reaches an endpoint it did not script has found
something, and quietly handing it a `200` would hide it.

What is *not* in the fixtures crate: the fakes that speak lemonfiber's own models
rather than a port — the Servarr service and the VPN gateway. Those need the
seeding and diagnosis models to say anything, so they stay beside the integration
tests that use them.

## Where an adapter's error becomes an operator's problem

Adapters return their port's typed `Failure`. Turning that into something an
operator can act on happens through `Diagnose`, at the point where the failure is
handled rather than where it is raised — the same missing binary is a blocking
error during setup and an absent version field in a version report.

See [error-model.md](error-model.md).

## Related

- [module-layout.md](module-layout.md) · [dispatch.md](dispatch.md)
