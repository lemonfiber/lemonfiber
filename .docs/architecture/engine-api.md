# The Engine API

What lemonfiber asks the container engine, and how the answers are tested
without a daemon.

Why reads go through the Engine API and writes go through Compose is
[ARCH-R14](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md),
and the seam itself is in [ports-and-adapters.md](ports-and-adapters.md). This
page is about the adapter: `adapters::docker::Daemon`, on `bollard`.

## What it asks

| Port method | Engine route | Why not Compose |
|-------------|--------------|-----------------|
| `list` | `GET /containers/json` | One poll per second across nineteen services |
| `logs` | `GET /containers/{id}/logs` | Streams, and Compose cannot narrow to a service list |
| `stats` | `GET /containers/{id}/stats` | Compose has no equivalent |
| `exec` | `POST /containers/{id}/exec` | The leak test runs the same command in two namespaces |

## The client is built on first use

Not at construction. The API version has to be settled with the daemon before
anything is asked of it, and settling it is itself a request — so a constructor
that could not fail would have to guess a version, and one that could fail would
turn an absent daemon into a startup error. An operator whose Docker Desktop is
still starting runs `lemonfiber config show` and expects an answer.

The failure is deliberately **not** remembered. A daemon being down is a
condition that ends, and an adapter that cached the first refusal would keep
reporting it long after Docker Desktop finished starting.

### What version negotiation actually buys, today

`negotiate_version` asks the daemon for its API version. In `bollard` 0.21 the
result does not reach the wire: the request URI is built as
`…/v1.44/containers/json` and then passed through `Url::join("/containers/json")`,
and joining an absolute path **replaces** the path rather than extending it. Every
request therefore goes out unversioned.

That is harmless — an unversioned path gets the daemon's own default, which is
the most compatible thing that could happen — and it is not what the code
appears to do, which is why it is written down here. The call is kept because it
is also the cheapest liveness check available: it turns "the engine is not
there" into a clean `Unreachable` at the first call rather than a decode failure
somewhere further in.

## Correlation, and what the summary does not carry

Containers are matched back to services by Compose's own labels —
`com.docker.compose.project` and `com.docker.compose.service` — rather than by a
naming convention this code would have to keep in step with. The listing is
filtered by label **at the engine**, so a machine running several stacks does
not send nineteen containers over the socket to have eighteen discarded.

Two things the container summary does carry, since API 1.44: a typed `State` and
a typed `Health`. Neither needs parsing.

One thing it does not: the **exit code**. It appears only inside the
human-readable status line, `Exited (137) 2 hours ago`, so that is where it is
read from. The alternative — inspecting each container — is one request per
service per refresh, which at a dashboard's rate is nineteen requests a second
to learn one number.

## Streams become channels

One task per container, each owning its stream and sending owned values. A
service whose stream stalls then delays its own panel and nothing else. A closed
channel is the reader having moved on, which is ordinary — the panel was closed
— and needs no handling beyond stopping.

The channel is bounded. A reader that has stopped reading should slow its
producer down rather than grow a buffer without limit.

## Testing it: an engine of our own

The adapter is the one part of `lemonfiber-core` a trait fake cannot exercise.
Its whole job is to speak a wire protocol, so a fake implementing `Engine` would
prove only that the fake works — the trait boundary sits *below* the code under
test.

So the **daemon** is what gets replaced. `crates/lemonfiber-core/tests/engine.rs`
carries a socket that answers the Engine API with whatever a test wants to say,
which drives the connection, the request, the decoding and the mapping in one
pass. It needs no Docker installed, which matters: a test that required a real
daemon would make the coverage gate depend on what happens to be running.

It answers three shapes:

| Shape | Used by |
|-------|---------|
| A JSON body under a status code | `list`, `stats`, exec creation and inspection |
| Docker's multiplexed framing — eight-byte header, then payload | `logs` |
| The same framing behind a `101` protocol upgrade | `exec` output |

It lives in `tests/` rather than `src/` because it is scaffolding rather than
product, and because scaffolding held to full line coverage grows tests about
the scaffolding.

Two things it taught, both cheaper to read than to rediscover:

- Header names are matched **case-insensitively**. The client sends
  `content-length` in lower case, and matching the specification's spelling
  instead is a request body silently never read.
- Tests stop the engine and wait for it, rather than walking away. A socket
  still being served while the next test binds its own is a flake that will not
  reproduce.

## Related

- [ports-and-adapters.md](ports-and-adapters.md) — the seam, and why `Receiver`
- [module-layout.md](module-layout.md) — where this sits
- [error-model.md](error-model.md) — how a refusal reaches an operator
