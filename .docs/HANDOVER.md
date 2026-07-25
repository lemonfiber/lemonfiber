# Handover — 2026-07-25

Where the work stopped, what is left of M2, and the things that will cost you an
hour if you rediscover them yourself.

Delete this file when M2 closes.

## State

`main` has the full M2 skeleton and most of its deliverables. [#19](https://github.com/lemonfiber/lemonfiber/pull/19)
was open and merging when this was written — check it landed before branching.

| M2 deliverable | State |
|----------------|-------|
| Workspace + `cargo-dist` scaffold | done |
| `stack.toml` parser + validation | done — 12 rules, checked at load |
| Embedded assets, `--stack-dir` | done, validated at build time by `build.rs` |
| Platform detection | component done; **probes outstanding** ([#3](https://github.com/lemonfiber/lemonfiber/issues/3)) |
| Compose command builder | done, golden-tested per form |
| Form closure + composition | done |
| `up` / `down` / `restart` / `pull` | done |
| **`ps` / `logs`** | **not started — needs the engine adapter** |
| `.env` read/write | done, plus `config get/set/show` |

**M2's exit criterion is met.** `lemonfiber up tv` and a hand-written
`docker compose` invocation resolve to the same eight services with identical
definitions. The check is
`scratchpad/exit_criterion.py` in the session that wrote this; it is short enough
to rewrite — resolve both through `docker compose config --format json` and
compare the models, because comparing command lines proves nothing.

## The one thing left

**The engine adapter**, and `ps` / `logs` on top of it.

`ports::docker::Engine` is already defined with `list`, `exec`, `stats` and
`logs`. Nothing implements it. `adapters/docker.rs` does not exist yet, and the
architecture test already reserves that filename — `bollard` is permitted there
and nowhere else, so a first attempt to use it anywhere else fails the build.

Why it is worth doing next: it unblocks health-gating `up` (`B2-R1`), the VPN
leak test, the platform probes in [#3](https://github.com/lemonfiber/lemonfiber/issues/3),
and `ps`/`logs` — which the M2 row lists but which **cannot** go through Compose.
`ARCH-R14` puts reads on the Engine API, and an earlier PR had to be corrected
for exactly that.

Streams come back as `tokio::sync::mpsc::Receiver`, not `impl Stream`. That is
deliberate; see `.docs/architecture/ports-and-adapters.md`.

## Things that will cost you time if you rediscover them

**The coverage gate is 100% of lines, and it has no escape hatch.**
`#[coverage(off)]` is nightly-only, so an uncoverable line cannot be excluded —
it has to not exist. Three patterns produce them, all written up in
[`architecture/error-model.md`](architecture/error-model.md):

- `let Ok(x) = … else { unreachable!() }` — the else arm
- `assert!(cond, "{} …", value.method())` — the argument only evaluates on failure
- `.map_err(|e| …)` where nothing triggers the failure — a closure is a function

**When the coverage report contradicts itself, the profile is stale.**
`cargo llvm-cov clean --workspace` does not always clear it. A full `cargo clean`
does. The symptom is a summary claiming missed lines that `--text` shows as
covered, or mangled names for closures you just deleted. To ask one profile
several questions without re-running tests between them:

```sh
cargo llvm-cov --no-report --workspace
cargo llvm-cov report --summary-only
cargo llvm-cov report --lcov --output-path /tmp/cov.info
```

This cost most of an afternoon. Do not debug coverage without a clean first.

**The gate keeps finding untested rules, not unreached lines.** Every time so
far it has surfaced behaviour nothing asserted: the composability rule, duplicate
form and service ids, `Failure::Unusable`, and six error paths in `config`. Treat
a gate failure as a question about what is untested, not as a number to move.

**Some rules cannot be exercised by the real stack.** Every form in media-stack
is composable, so "must run alone" is proven against a small inline fixture in
`stack/closure.rs`. Expect to need more of those.

## Repo conventions that are easy to get wrong

- **Spec first, always.** A gap found while building gets a spec PR before the
  implementation. Three landed this way today (`OPS-R26`, `ARCH-R42`, `B1-R15`).
- Commits: conventional subject, prose body explaining *why*, `Spec:` trailer,
  `git commit -s`, **no AI attribution**.
- Requirement identifiers **never** appear in code comments — an architecture
  test fails the build. They go in commits and PR bodies.
- No `#[allow]` anywhere in `src/`. Change the code or change the workspace lint.
- `schema_version` does **not** increment before the first release candidate
  (`ARCH-R43`). The schema changes in place.

## Known gaps, deliberately left

- **`Protocols` is recorded, not verified.** `config set LEMONFIBER_USENET on`
  records an answer; nothing checks a provider actually works. That is `init`
  (M3) and credential validation (`A3`).
- **`up` is not health-gated.** Needs the engine adapter.
- **The daemon flavour is guessed.** `Environment::resolve(HOST_OS, false)` in
  `main.rs` assumes not-Desktop, so Linux Desktop is misidentified. Nothing
  depends on it yet, and the comment says so. The adapter fixes it.
- **Compose-parity validation stays in media-stack.** lemonfiber has no YAML
  dependency and no reason to gain one; recorded on
  [#1](https://github.com/lemonfiber/lemonfiber/issues/1).

## Where to read first

`.docs/architecture/` — `module-layout.md`, then `dispatch.md`, then
`ports-and-adapters.md`. The error model and the coverage patterns are in
`error-model.md`, and they are the two things most likely to slow a first change
down.
