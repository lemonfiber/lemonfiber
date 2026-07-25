# The embedded stack

How the stack gets into the binary, and why an incompatible pairing cannot
compile.

Why the stack is embedded at all, and how the three versions relate, are in the
spec's [versioning contract](https://github.com/lemonfiber/spec/blob/main/20-architecture/contracts/versioning.md).
This is the mechanism.

## Three places it is checked

| When | Where | Catches |
|------|-------|---------|
| Build | `crates/lemonfiber/build.rs` | A missing submodule, or a stack this build cannot read |
| Load | `core::stack::Source::manifest` | An external `--stack-dir` that is missing or from another generation |
| Parse | `lemonfiber_manifest::Manifest::from_toml` | The schema-version gate itself |

All three use the same parser, so they cannot disagree about what "readable"
means. `build.rs` takes `lemonfiber-manifest` as a **build-dependency** for
exactly that reason — a second, simpler check written in the build script is how
the two drift.

## What the build refuses

Verified by hand; both messages are the ones an operator or contributor sees.

Submodule not initialised:

```
error: the embedded stack is missing at …/assets/media-stack
error:
error: It is a git submodule, and a fresh clone does not populate it.
error:
error:   git submodule update --init --recursive
```

A stack from a generation this build cannot read:

```
error: the embedded stack cannot be read by this build
error:
error: the manifest declares schema version 99, and this build reads [1]
error:
error: Manifest: …/assets/media-stack/stack.toml
error: The submodule and this binary are out of step; move the pin, or
error: teach the parser the generation the stack now declares.
```

Written as `cargo::error=` lines rather than a panic, so the message survives
Cargo's framing instead of arriving inside a backtrace. `refuse` returns `!` and
calls `exit(1)`, which keeps it usable in a `let … else`.

Paths are resolved from the workspace root rather than by walking up out of the
crate, so a reader sees `assets/media-stack` and not
`crates/lemonfiber/../../assets/media-stack`.

## `Source` — embedded or external, one type

```rust
pub enum Source {
    Embedded(&'static Dir<'static>),   // include_dir!, from the binary
    External(&'static Path),           // --stack-dir
}
```

`Copy`, so it is passed by value and lives on `Ctx` without ceremony. Everything
above it reads a manifest without knowing which variant it has.

`include_dir` sits in `lemonfiber-core`, not only in the binary. It is not a
rendering crate, so the boundary that matters is unaffected, and putting the
variant in the core is what lets the two readings be tested side by side:

```rust
assert_eq!(
    Source::Embedded(&EMBEDDED).manifest_text().ok(),
    checked_out().manifest_text().ok(),
);
```

The binary's `include_dir!` and the test's point at the same submodule, so that
assertion would fail if embedding ever diverged from the checkout.

### Why the path is leaked

```rust
Source::External(Box::leak(path.into_boxed_path()))
```

`Source` is `'static` so it can be `Copy` and live on a context shared across
tasks. The alternative — a lifetime parameter on `Source`, and therefore on
`Ctx`, and therefore on everything holding one — costs more than one allocation
that lives as long as the process was going to anyway.

## Failure is about the pairing, not the syntax

`Failure::Unusable` deliberately does not say "parse error". A stack from another
generation parsed *fine*; it is the combination that does not work:

> This stack was written for a different version of lemonfiber

Reporting it as a syntax error would send someone to look for a typo in a file
that has none.

`Failure::NotEmbedded` is `Severity::Critical` and `State::Unknown` — the build
is supposed to make it impossible, so reaching it means the binary was assembled
by something other than its own build, and guessing at a remedy would be
inventing one.

## What is not here yet

Materialising the stack's compose files to disk. `Source` reads the manifest
today; writing the fragments out arrives with the compose driver, which is the
first thing that needs them.

## Related

- [module-layout.md](module-layout.md) · [dispatch.md](dispatch.md)
- [error-model.md](error-model.md)
