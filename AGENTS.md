# AGENTS.md — lemonfiber

> **Start at the roadmap and board on [lemonfiber.app](https://lemonfiber.app),
> rendered from the report of where every unreleased version stands. Then the
> rules** every repository shares:
> [working in the repositories](https://github.com/lemonfiber/spec/blob/main/50-governance/working-in-the-repositories.md)
> and [the rules for agents](https://github.com/lemonfiber/spec/blob/main/50-governance/ai-contributors.md).
> This file holds only what is true of this repository.

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

0. [`status/`](status/) — one row per requirement this repository has worked on, a
   file per feature, with the code and the test that hold it. Change it in the same
   PR as the work it describes; `spec-check` holds every row to the spec and this
   tree.
1. The three `30-repos/lemonfiber*.md` specs above.
2. [`20-architecture/component-model.md`](https://github.com/lemonfiber/spec/blob/main/20-architecture/component-model.md) — the crate boundaries and async model.
3. The feature you're implementing under `10-functional/features/`.

## Code standards (enforced)

- `unsafe` is **forbidden** crate-wide. No `unwrap`/`expect`/`panic`/`todo` in
  non-test code (`Q-R12`). Library errors are typed and carry a remedy.
- **No lint suppressions in `src/`** — change the code or the rule, never
  `#[allow]`. An arch test fails on it.
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

`just coverage` is the line `sonar` runs, character for character,
`--no-fail-fast` included; `just ci` is everything the `build` job reads plus
spelling and the scripts, and the `justfile` names what it leaves out. When
coverage comes back red, run `just uncovered`: it re-reads the profile already
gathered, naming the lines first and then the functions the run never entered.

## Working in a worktree

Every worktree under `~/Development/lemonfiber` shares one build cache, deliberately.
It is set by a `.cargo/config.toml` in the directory *above* the checkout rather than
by anything in this repository: cargo reads that file from the current directory
upward, so one copy covers every clone and worktree beneath it, and it names an
absolute path on one machine, which is why no repository can carry it.

The reason is disk: a full workspace build is 10–14 GB, and cargo reclaims none
of it. The cost is that builds in different worktrees serialise on cargo's lock.

**Remove your worktree once your PR merges.** That cache is sized for a handful of
them. Cargo hashes a package id relative to the workspace root, so two worktrees
produce byte-identical artifact names and each build overwrites the last one's work.
Past a handful, every build in every worktree is a cold build, and nobody can see
why from inside their own.

A test reading a repo file through `CARGO_MANIFEST_DIR` — the parity table, the
generated-artefact checks — can read *another worktree's* copy of it. Force a
rebuild (`touch` the test source, or `cargo clean -p <crate>`) before believing a
surprising result.

**A whole-suite run can hang before it runs anything, and `--test <name>` is the way
through.** `cargo nextest run` enumerates every test binary first, and enumerating
sixty of them while a sibling worktree rewrites the same artefacts leaves processes
parked at 0% CPU in `_dyld_start`, before `main`. Naming one binary avoids it:
`cargo nextest run -p lemonfiber --test withholding`, or `-p <crate> --lib` for the
unit tests.
