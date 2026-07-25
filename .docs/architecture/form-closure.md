# Form closure and command construction

How named forms become a `docker compose` invocation.

Why closure is written out rather than inferred, and why narrowing comes second,
are in the spec's [stack manifest contract](https://github.com/lemonfiber/spec/blob/main/20-architecture/contracts/stack-manifest.md)
and [data flow](https://github.com/lemonfiber/spec/blob/main/20-architecture/data-flow.md).
This is the implementation.

## Two steps, in this order

```rust
resolve(&manifest, &forms, protocols) -> Result<Plan, Failure>
build(&plan, &settings, stack, &action, environment) -> Vec<String>
```

**Closure** is a union. A form's `profiles` list is the complete set it needs,
written out in the manifest, so resolving it never requires knowing what a
service does. Naming the same form twice changes nothing; naming two forms is
the union of their lists.

**Narrowing** removes profiles whose protocol is not configured.

The order is load-bearing and there is a test that says so by name. `tv` needs
`search`, `subs` and `tv` regardless of protocol; narrowing first would have
nothing to narrow against and would drop them.

## Which profiles are guarded is the manifest's answer

```toml
[[profile]]
id       = "torrent"
protocol = "torrent"
```

Narrowing reads the field. Nothing in this crate knows that a profile called
`torrent` is special, so a stack that renames its download profiles keeps
working.

That was not true when this was first written. The contract had no way to say
which profiles need a provider, so the two ids were constants in this module —
the only per-service knowledge in Rust anywhere. A fork renaming either would
have kept parsing, kept resolving, and quietly stopped being narrowed: a torrent
profile starting on a machine with no VPN configured.

It is worth knowing *why* it was worth a spec change rather than a comment. The
failure was silent and pointed the dangerous way — an unguarded tunnel rather
than a missing service — and silent failures that fail unsafely are the ones to
spend a contract change on.

## Why `--project-directory` and `--file` are both passed

```
docker compose --project-name lemonfiber \
  --project-directory /opt/lemonfiber/stack \
  --file /opt/lemonfiber/stack/compose.yml \
  --profile media up --detach
```

The stack's fragments carry `project_directory: .` deliberately: without it a
fragment's relative paths resolve against `compose/` rather than the project
root, and `./config/sonarr` silently becomes `compose/config/sonarr`. Passing
the file without also naming the directory reintroduces exactly that, from a
different direction.

`--project-name` is not cosmetic either — it is what Compose stamps on every
container as a label, and those labels are how the Engine API reads are
correlated back to the services that declared them.

## Profiles are sorted

`Plan.profiles` is a `BTreeSet`, so the same request always produces the same
command. Without that, golden files would test insertion order, and Compose
would see a "different" project on each invocation.

## Golden files

`crates/lemonfiber-core/tests/golden/up_<form>.txt` — one per form, and a test
asserts the set of files matches the set of forms exactly. A form without a file
is untested; a file without a form is asserting something the stack no longer
declares.

Each form is checked on **all four environments against the same file**. That
the four agree is a fact about today, not a rule: `build` takes `Environment`
already, and when one first changes the command its assertion fails and that
form's file splits. Writing four identical files now would hide that moment
rather than catch it.

Regenerate after an intended change, then *read the diff*:

```sh
LEMONFIBER_BLESS_GOLDEN=1 cargo test -p lemonfiber-core --test golden
```

A golden file updated without being read is a test asserting whatever the code
happens to do.

### Checked against real Compose

The generated invocations were run through `docker compose … config --quiet`
against the real media-stack checkout for `library`, `search`, `tv` and `full`;
all four resolve. The golden files pin the argv, and that check is what says the
argv is the *right* one — worth repeating by hand when the shape changes.

## `environment` is threaded but unused

`build` takes it and currently ignores it. Deliberate: the first change that
needs per-environment behaviour should be about that behaviour, not about
threading a parameter through every call site and every test.

## Related

- [module-layout.md](module-layout.md) · [embedded-stack.md](embedded-stack.md)
- [error-model.md](error-model.md)
