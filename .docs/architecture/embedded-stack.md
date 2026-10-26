# The embedded stack

How the stack gets into the binary, and why an incompatible pairing cannot
compile.

Why the stack is embedded at all, and how the three versions relate, are in the
spec's [versioning contract](https://github.com/lemonfiber/spec/blob/main/20-architecture/contracts/versioning.md).
This is the mechanism.

## Three places it is checked

| When | Where | Catches |
|------|-------|---------|
| Build | `crates/lemonfiber-bundled/build.rs` | A catalogue that is not populated, a copy that is not the manifest its pin names, or an assembled stack this build cannot read |
| Load | `core::stack::Source::manifest` | An external `--stack-dir` that is missing or from another generation |
| Parse | `lemonfiber_manifest::Manifest::from_toml` | The schema-version gate itself |

All three use the same parser, so they cannot disagree about what "readable"
means. `build.rs` takes `lemonfiber-manifest` as a **build-dependency** for
exactly that reason — a second, simpler check written in the build script is how
the two drift.

## What the build assembles

The stack is the one the default bundle in `assets/plugins` declares: the bundle's
own tables, a service for each part it assembles, and for each part it installs the
service its copy says it takes the place of, until the part moves. Each part's
starting configuration and every recording a claim names come from its copy; the
licence comes from the catalogue.

## What the build refuses

`refuse` prints what failed, why, and the directory it was reading, then exits:

```
error: the embedded bundle cannot be read

<why>

At: …/assets/plugins
If the directory is empty, the submodule is not populated:
  git submodule update --init --recursive
```

The same shape says when the bundle's stack cannot be assembled, or assembles into
a stack this build does not read. `refuse` returns `!` and calls `exit(1)`, which
keeps it usable in a `let … else`.

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

The binary's `include_dir!` and the test's point at the same carried tree, so that
assertion fails if embedding ever diverges from it.

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

## Writing it to disk

Compose reads files, so an embedded stack is written out before it runs:
`Source::materialise` writes it on every invocation rather than caching it, so no
stale copy survives an upgrade, and returns an external stack's path as it is.
[`materialised.rs`](../../crates/lemonfiber-core/src/materialised.rs) keeps a
checksum of each file lemonfiber last wrote, so a later write can tell an
operator's edit from a version they have not upgraded yet.

## Related

- [module-layout.md](module-layout.md) · [dispatch.md](dispatch.md)
- [error-model.md](error-model.md)
