# Architecture (repo-local)

How the subsystems are built in Rust. The *what* and *why* are in the spec's
[20-architecture](https://github.com/lemonfiber/spec/blob/main/20-architecture/);
this covers implementation detail only — the decisions that are ours because they
are about Rust rather than about the product.

| Page | Covers |
|------|--------|
| [module-layout.md](module-layout.md) | Where things live, and what the architecture test enforces |
| [dispatch.md](dispatch.md) | The one entry point every surface goes through |
| [ports-and-adapters.md](ports-and-adapters.md) | The seam to the outside world, and how to fake it |
| [engine-api.md](engine-api.md) | What the container engine is asked, and testing it without a daemon |
| [error-model.md](error-model.md) | Typed errors, one operator-facing value, and how the coverage gate proves it |
| [embedded-stack.md](embedded-stack.md) | How the stack gets into the binary, and why a bad pairing cannot compile |
| [form-closure.md](form-closure.md) | Forms to profiles to a Compose invocation, and the golden files |
| [storage-probe.md](storage-probe.md) | Proving hardlinks on the real data root, and staying testable without a filesystem port |

Planned as their subsystems land: `render-loop.md`, `vpn-port-forwarding.md`,
`seed-clients.md`, `compose-construction.md`.

## The rule of thumb

If a reader needs it to change this code, and it is not a decision about what the
product does, it belongs here. If it is a decision about what the product does, it
belongs in the spec and this page links to it.

Code links here. These pages cite the spec. Code never cites the spec directly —
an architecture test enforces that.
