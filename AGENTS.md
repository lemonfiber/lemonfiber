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

- `just rebased` after every rebase. It syncs the submodule and builds every target.
  Both halves are failures a rebase hides rather than reports: the stack is a submodule
  and a rebase across a commit that moved it leaves the old one checked out, which
  surfaces as manifest tests failing about a fixture; and a file that merely *uses* an
  interface you changed conflicts with nothing, so it merges clean and then does not
  compile.
- `cargo fmt --all`, then clippy and the tests for what you actually touched.
  `just test` runs the suite through `nextest`, which is about three times quicker
  than `cargo test` here.
- Your change cites a spec identifier in a commit `Spec:` trailer and the PR body.
- Behaviour change? The spec PR merged first.
- The [definition of done](https://github.com/lemonfiber/spec/blob/main/40-quality/definition-of-done.md) is met.

**Push, and let CI run the slow gates.** `just ci` and `just coverage` are the same
commands CI's own jobs run — `sonar` runs that coverage line character for character
— and CI runs them on an exclusive build cache, in parallel with twenty-odd other
checks. Running them here first tells you nothing CI will not tell you sooner, and
costs ten minutes and more of a shared machine. Reach for one locally only when a
named check comes back red, and when that check is coverage use

```
cargo llvm-cov report --ignore-filename-regex '<the skipped regex>' --show-missing-lines
```

which re-reads the profile already gathered rather than building and running the
whole workspace a second time.

## Working in a worktree

Every worktree under `~/Development/lemonfiber` shares one build cache, deliberately
— `.cargo/config.toml` says why, and names the cost. Two things follow from it.

**Remove your worktree once your PR merges.** That cache is sized for a handful of
them. Cargo hashes a package id relative to the workspace root, so two worktrees
produce byte-identical artifact names and each build overwrites the last one's work.
Past a handful, every build in every worktree is a cold build, and nobody can see
why from inside their own.

**Do not trust a local pass or a local failure while a sibling may be building.** The
same collision means a test reading a repo file through `CARGO_MANIFEST_DIR` — the
parity table, the generated-artefact checks — can read *another worktree's* copy of
it. A red run then names a file that is correct in your tree, and a green run is
green about somebody else's. Force a rebuild (`touch` the test source, or
`cargo clean -p <crate>`) and run it again before believing either answer.

## Commits

Conventional-commit style, a `Spec:` trailer, and **no AI attribution** — no
`Co-Authored-By`, no tool reference.
