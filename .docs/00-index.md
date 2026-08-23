# lemonfiber/.docs

Repo-local technical documentation — the Rust-specific *how*. Product decisions
live in the [spec](https://github.com/lemonfiber/spec); this tree holds only what
is specific to implementing them here.

Code links to these pages (never to spec requirement IDs, which are banned in
comments — `GOV-R6`). These pages, in turn, cite the spec.

| Area | Holds |
|------|-------|
| `architecture/` | How subsystems are built — the ports boundary, dispatch, the engine API, the embedded stack, the error model |
| `conventions/` | Naming, error style; the comment policy is canonical in the spec |

See the three-layer model in
[spec 40-quality/code-comments.md](https://github.com/lemonfiber/spec/blob/main/40-quality/code-comments.md).
