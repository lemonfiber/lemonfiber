# Error model

Two levels: typed errors for control flow, one value type for the operator.

What an error must contain is in the spec's
[error and remedy model](https://github.com/lemonfiber/spec/blob/main/10-functional/features/g-ux/g4-error-model.md).
This is how it is built in Rust.

## The two levels

**Typed errors** — a `thiserror` enum per subsystem, named `Failure`, used for
matching and control flow. `ports::process::Failure`, `ports::docker::Failure`,
`ports::service::Failure`.

**One operator-facing value** — `error::Problem`. What happened, what it means,
what to do, and where to look, as fields:

```rust
pub struct Problem {
    pub code: Code,
    pub severity: Severity,
    pub state: State,
    pub summary: String,        // what happened
    pub meaning: String,        // what it means for them
    pub remedies: Vec<Remedy>,  // what to do
    pub detail: Option<String>, // where to look
    pub cause: Option<Box<Problem>>,
}
```

The bridge is one trait:

```rust
pub trait Diagnose {
    fn problem(&self) -> Problem;
}
```

## A problem without a remedy will not compile

`Problem::new` takes a `Remedy` as a required argument. There is no constructor
that produces an empty `remedies`.

For the case where nothing is known, `Problem::unknown` sets `State::Unknown` and
attaches the support-bundle escalation, so the four parts still hold. Admitting
ignorance costs the operator a bundle; confident wrong guidance costs them an
afternoon and costs us the trust that made them believe it.

This is the whole trick. "Every error carries a remedy" is a rule that erodes one
message at a time when it is a convention, and cannot erode when it is an
argument.

## Why the coverage gate is the exhaustiveness proof

Rust cannot enumerate enum variants, so the obvious test — "every variant of
every error produces a remedy" — has no way to iterate. The usual answer is a
hand-maintained list, which rots.

The 100% line coverage gate on applicable code does it instead. Every `match` arm
in every `Diagnose` implementation must execute in some test for the gate to
pass, and each of those arms constructs a `Problem`, which cannot be constructed
without a remedy. A new variant with no test fails the gate, not review.

Each error module also carries an explicit sweep over all its variants, asserting
the message names its subject and the problem carries a remedy. The gate proves
the arms ran; the sweep proves they said something useful.

## Writing assertions the gate can reach

Two patterns leave lines no passing test can cover, and both look harmless:

```rust
let Ok(value) = thing() else { unreachable!("cannot happen") };   // the else arm
assert!(cond, "{} is wrong", path.display());                     // the argument
```

A `let … else` needs an arm for the case that cannot happen, and an argument
expression in a failure message only evaluates when the assertion fails. Prefer
comparing whole values, and inline captures:

```rust
assert_eq!(thing().ok(), Some(expected));
assert!(cond, "{path:?} is wrong");
```

A third is subtler: a closure that only runs on failure.

```rust
std::fs::write(path, text).map_err(|err| Failure::NotWritten { .. })?;   // closure
if let Err(err) = std::fs::write(path, text) { return Err(..) }          // no closure
```

A closure is a function of its own as far as coverage is concerned, and one that
only runs when something fails is a symbol no passing test reaches. `map_err` is
fine where a test triggers the failure; where it cannot, prefer `if let Err`.
`unwrap_or_else(|| …)` has the same shape — use `unwrap_or(…)` when the fallback
is cheap.

A fourth is not about assertions at all. **A block whose last statement is an
`await` never has its closing brace marked executed.**

```rust
if let Some(mut lines) = opened {           // the `}` below is uncoverable
    while let Some(line) = lines.recv().await { … }
}

let mut lines = opened.unwrap_or(closed);   // no wrapping block, no orphan brace
while let Some(line) = lines.recv().await { … }
```

Build the fallback eagerly and drop the wrapper. An already-closed channel or an
empty collection costs nothing and removes the shape entirely.

## A file may only have one coverage mapping

`#[cfg(test)] mod tests` inside a file that an **integration test also reaches**
compiles that file twice: once with `cfg(test)` for the unit-test binary, once
without for the integration-test binary. The two coverage mappings do not
describe the same line set, and merging them reports missed lines that no
annotated report can point at — `--summary-only` says two, `--text` shows none,
and nothing you change moves the number.

The symptom is unmistakable once seen: **the count is non-zero and the annotated
report is empty.** When they disagree like that, do not go looking for the line.
Look for a second binary reaching the same file.

The fix is to pick one. Where the file is an adapter, the integration test is
the right home anyway — driving it through its port tests behaviour rather than
internals, and a test that reaches past the port cannot tell a wrong mapping
from wrong behaviour. See [engine-api.md](engine-api.md).

Neither is a style preference. Per-item coverage exclusion needs
`#[coverage(off)]`, which is nightly-only, so an uncoverable line is a gate
failure with no escape hatch.

## When the coverage report contradicts itself

It will, and the cause is almost always **stale profile data**. `cargo llvm-cov
clean --workspace` does not always clear it, and a report built from a mix of old
and new profiles names symbols that no longer exist in the source.

The symptom is a summary claiming N missed lines while `--text` shows a clean
file, or mangled names for closures you just deleted. The fix:

```sh
cargo clean && cargo llvm-cov --workspace --ignore-filename-regex '(main\.rs)' --fail-under-lines 100
```

To ask the same profile more than one question without re-running the tests —
which is what makes the answers disagree — run once and query it repeatedly:

```sh
cargo llvm-cov --no-report --workspace
cargo llvm-cov report --summary-only
cargo llvm-cov report --lcov --output-path /tmp/cov.info
```

## `Code` is a newtype, not an enum

```rust
pub const MISSING_PROGRAM: Code = Code::new("PROC-1");
```

Declared as a `const` beside the error that raises it. An enum would centralise
every code in one file, far from the code that uses it, and turn adding an error
into editing a shared list — the kind of file that collects merge conflicts and
stops being read.

Codes are never recycled. An operator who searches for one should find the same
answer a year later.

## The inventory is read out of the source

What that decision costs is enumeration: there is no registry, so nothing can list
the codes at run time. `reference/error-codes.md` is therefore read from the
declarations themselves, by `lemonfiber::codes` — a lexer over `crates/*/src` that
tells code from a string from a comment, drops what only the tests compile, and
sorts what is left by family and number. `just codes` rewrites it; a test compares
the committed bytes with a fresh read.

The reader refuses to guess. A `Code::new(…)` whose name is not a literal, and a
file whose braces do not balance by its last line, are both reported by name and
line rather than left out — a list that is quietly short is the one way the artefact
could be wrong while still agreeing with itself.

Two things follow from reading it this way. A code is only found where it is written
as a `const` call, so text that merely looks like a code — a bitrate, an encoding —
is never mistaken for one. And the architecture test that no two problems share a
code reads through the same function, so what counts as a declaration is decided
once rather than twice.

## Severity is `Ord`

`Critical > Error > Warning > Advisory`, so a health summary can take a maximum
without a comparison table. The declaration order in the enum is therefore
load-bearing: variants are declared low to high.

## Cause, not a chain of symptoms

`cause: Option<Box<Problem>>` attributes several failures to one root. A full
disk producing eleven failures is one problem with eleven symptoms, and reporting
it as eleven is how an operator is led to fix the wrong thing repeatedly.

## Where a typed error becomes a problem

At the point where it is *handled*, not where it is raised. The same
`Failure::NotFound` is a blocking error during setup and an absent field in a
version report — only the caller knows which.

That is why `Diagnose` is a separate trait rather than a `From` impl: a `?` that
silently converted would decide the severity at the wrong place.

## The core never formats

`Problem` has no `Display`. Rendering is the surface's job — the core cannot
print, and a `Display` impl would be a rendering that all three surfaces would
quietly start depending on.

`Code` does implement `Display`, because a code is one token with one correct
form and no layout decision to make.

## Related

- [dispatch.md](dispatch.md) — how a problem reaches a surface
- [ports-and-adapters.md](ports-and-adapters.md) — where typed errors come from
