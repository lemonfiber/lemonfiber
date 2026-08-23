# AGENTS.md — lemonfiber

Guidance for any AI agent (Cursor, Codex, Aider, Claude Code, …) working in this
repo.

> **Common rules for every lemonfiber repo are canonical in the spec:**
> [50-governance/ai-contributors.md](https://github.com/lemonfiber/spec/blob/main/50-governance/ai-contributors.md).
> Read them. This file is the `lemonfiber`-specific header only.

## What this repo is

The `lemonfiber` binary — CLI and TUI over one core, plus the local HTTP API the
web surface draws. Rust workspace. See
the spec: [`30-repos/lemonfiber.md`](https://github.com/lemonfiber/spec/blob/main/30-repos/lemonfiber.md),
[`30-repos/lemonfiber-tui.md`](https://github.com/lemonfiber/spec/blob/main/30-repos/lemonfiber-tui.md),
[`30-repos/lemonfiber-reference.md`](https://github.com/lemonfiber/spec/blob/main/30-repos/lemonfiber-reference.md).

## The one rule you cannot break here

**`lemonfiber-core` has no UI dependency** — not ratatui, not clap, not an HTTP
server. It cannot print. Enforced by the crate graph and an architecture test
(`ARCH-R11`). If you find yourself adding a UI crate to `lemonfiber-core`, stop —
the logic and the rendering must stay separate.

## Where to start

0. [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md) — what is built vs. what
   the roadmap still asks for, so you don't reconstruct it from source. Keep it
   current in the same PR as your change.
1. The three `30-repos/lemonfiber*.md` specs above.
2. [`20-architecture/component-model.md`](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md) — the crate boundaries and async model.
3. The feature you're implementing under `10-functional/features/`.
4. Find the requirement your change serves **before** editing. Cite it.

## Code standards (enforced)

- `unsafe` is **forbidden** crate-wide. No `unwrap`/`expect`/`panic`/`todo` in
  non-test code (`Q-R12`). Library errors are typed and carry a remedy.
- **No lint suppressions in `src/`** — change the code or the rule, never
  `#[allow]`. An arch test fails on it.
- Comments explain *why*, never *what*; no lone one-liners (2–4 line blocks);
  **no requirement IDs in comments** (`GOV-R6`). Over-commenting is a defect.
- Repo-specific technical detail goes in [`.docs/`](.docs/), linked from code.

## Before you open a PR

- `just ci` passes (fmt, clippy `-D warnings`, test, typos, cargo-deny), and
  `just coverage` holds the 100% line that CI's sonar job enforces.
- Your change cites a spec identifier in a commit `Spec:` trailer and the PR body.
- Behaviour change? The spec PR merged first.
- The [definition of done](https://github.com/lemonfiber/spec/blob/main/40-quality/definition-of-done.md) is met.

## Commits

Conventional-commit style, a `Spec:` trailer, and **no AI attribution** — no
`Co-Authored-By`, no tool reference.
