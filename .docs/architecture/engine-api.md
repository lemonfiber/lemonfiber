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
| `located` | `POST /containers/create` | See below — there is no route that stats a host path |

### Asking a daemon about a path on its own machine

The last row is not a read of anything, and it is worth spelling out. The
pre-flight that refuses to start a stack on a machine without the location it
mounts needs one fact — does this path exist *there* — and the Engine API has no
route that answers it. Everything it touches on the host, it touches through a
container.

So the question is asked as a request to create one, against an image that
cannot exist (`sha256:` and sixty-four zeroes), with the path as the source of a
bind mount. A daemon validates the mounts before it looks for the image, which
makes the refusal the answer:

| Answer | Means |
|--------|-------|
| `400` … `bind source path does not exist` | The path is not on that machine |
| `404 No such image` | It is — the request got past the mount to reach the image |
| anything else | Neither; the asking did not work |

Nothing is created on either path, nothing is pulled, no image need be present on
the far machine, and there is nothing to remove afterwards: both outcomes are
failures. That is the reason for this shape rather than one that makes a real
container and removes it, and it is why the check costs the same over `tcp://` as
over `ssh://`.

**The ordering is the daemon's, not a promise.** If a release ever validated the
image first, every path would answer `404` and read as present. The caller in
`core::app::engine::remote` therefore refuses to believe a positive until a
second question — about a name that cannot be there — comes back negative from
the same daemon. The reading lives here, in `adapters::docker::presence`; the
decision about whether to trust it lives in the core, where a test can drive an
instrument that has stopped measuring.

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

So the **daemon** is what gets replaced. `crates/lemonfiber-adapters/tests/fake/`
carries a socket that answers the Engine API with whatever a test wants to say,
which drives the connection, the request, the decoding and the mapping in one
pass. It needs no Docker installed, which matters: a test that required a real
daemon would make the coverage gate depend on what happens to be running.

It is a module of its own rather than one inside the file that first needed it,
because two binaries drive the adapter — `tests/engine.rs` for listings and
streams, `tests/exec.rs` for running a command inside a container — and two
engines that were meant to answer the same way are two engines that will
eventually not.

It answers four shapes:

| Shape | Used by |
|-------|---------|
| A JSON body under a status code | `list`, `stats`, exec creation and inspection |
| Docker's multiplexed framing — eight-byte header, then payload | `logs` |
| The same framing behind a `101` protocol upgrade | `exec` output |
| The same upgrade, with one frame promising more than it delivers | a stream cut mid-chunk |

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

## Which daemon, and why it is decided once

The adapter does not read the environment. It is handed a `ports::docker::Target`
— a resolved endpoint, how it came to be chosen, and what may be shown of it —
and builds its client from that and nothing else.

That is a correction rather than a preference. Compose is a subprocess with an
inherited environment, so it obeyed `DOCKER_HOST` and a named context. This
adapter called `connect_with_local_defaults()`, which honours `DOCKER_HOST` only
when it names a unix socket and silently falls back to the local daemon
otherwise, and read `~/.docker/contexts` nowhere. On a laptop with a remote
context set, the reads described one machine and the writes changed another —
and on every machine where nobody had set one, the two agreed perfectly.

So there is one resolution, in `adapters::docker::context`, whose answer lands on
`Settings::docker`. The engine client is built from that field; the Compose
invocation names the same field as `docker --host … compose …`, which beats
anything the shell exported. Neither half can be aimed anywhere by itself.

`ports::docker::Target::refusal()` is the other half of the same idea. An
endpoint this build cannot drive — a scheme it does not speak, a TCP endpoint the
operator asked to have verified with TLS, a context this machine has no record of
— is refused by *both* halves before anything is attempted, because an endpoint
the reads cannot use must never become one the writes do.

### Telling refusals apart

For a socket on this filesystem there is one question: is Docker running. For a
daemon somewhere else there are several, and they have nothing in common. The
transport's whole chain of causes is read in `adapters::docker::refusal`, because
the condition is never in the outer error variant, and the distinctions are drawn
**only** for a remote target: `permission denied` from a local socket is a group
membership, and from an SSH endpoint it is a key.

## Related

- [ports-and-adapters.md](ports-and-adapters.md) — the seam, and why `Receiver`
- [module-layout.md](module-layout.md) — where this sits
- [error-model.md](error-model.md) — how a refusal reaches an operator
