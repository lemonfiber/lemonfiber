# `lemonfiber plugin`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber plugin`

```text
Read what a plugin may claim, where it may contribute, and how it is written.

For somebody writing a plugin rather than running a stack. Everything under this word is a read of a document this build publishes and attaches to every release: the capabilities a service can claim, the places a plugin may extend lemonfiber itself, and the schema an editor validates `plugin.toml` against.

Every one of them answers with no network, no catalogue and no stack running, and says which generation it is reporting — so an author who has to know whether a difference is their build or their manifest can tell.

Usage: lemonfiber plugin [OPTIONS] <COMMAND>

Commands:
  capabilities      List the capabilities a service can claim, and what claiming one undertakes
  extension-points  List the places a plugin may extend lemonfiber itself
  schema            Print the schema an editor validates `plugin.toml` against
  claims            Read a plugin's source and say what its claims come to
  provenance        Ask each image's registry whether anybody has said it is theirs
  help              Print this message or the help of the given subcommand(s)

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber plugin capabilities`

```text
List the capabilities a service can claim, and what claiming one undertakes.

A capability is a named, contracted thing a service can do, so that wiring can ask for one rather than name a service. Each carries the prose a claimant is held to, which bundled services already declare it, and the probes a claim has to bind — the vocabulary says what must be shown, and the claimant says where to ask.

A name here is the only kind a plugin may claim without namespacing it. A plugin's own capability is written `<plugin-id>:<name>` and is inert until something asks for it.

Usage: lemonfiber plugin capabilities [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber plugin extension-points`

```text
List the places a plugin may extend lemonfiber itself.

A point is a register lemonfiber already runs, and the point names where a plugin may put another row in it — never a hook and never code. Each says what a row carries, what is already standing in that register, and the capability a manifest has to ask for in order to contribute there.

Usage: lemonfiber plugin extension-points [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber plugin schema`

```text
Print the schema an editor validates `plugin.toml` against.

Generated from the types lemonfiber reads a manifest with, so it describes the reader rather than claiming something about it. Always machine-readable: it is a document for an editor rather than a listing for a person.

Usage: lemonfiber plugin schema [OPTIONS]

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber plugin claims`

```text
Read a plugin's source and say what its claims come to.

The three documents above say what may be written; this says what one manifest wrote. Everything it declares is held to the published schema, to the published vocabulary and to the published points in one pass — the fields, the kinds, the digest that fixes what runs, the paths, and the capabilities it asks of this build — every probe a claim binds is run against the recording it names, and each capability is reported as demonstrated, unproven or refuted rather than as claimed.

It then says what asking for each capability would come to on the stack this build pins: filled by one service, contested between several — which lemonfiber refuses to settle by install order — or inert, which is what a capability of the plugin's own is until something asks for it.

A refusal, or a claim its own recordings refute, exits non-zero. Nothing is installed, nothing is written, and no service is asked anything.

Usage: lemonfiber plugin claims [OPTIONS] <PATH>

Arguments:
  <PATH>
          The plugin's source: its directory, or the `plugin.toml` inside it

Options:
      --json
          Print machine-readable output

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```

## `lemonfiber plugin provenance`

```text
Ask each image's registry whether anybody has said it is theirs.

The one read here that reaches the network, and the only one: it asks the registry the manifest pins an image in whether it offers a signature for that exact digest, and says what this build makes of the answer.

Three answers and never two. An image whose signature verifies against a key you hold is **signed**. One nobody signed is **unproven** — a publisher who signed nothing has made no claim, which is a different fact from a claim that did not check out, and it does not stop an install. One carrying a signature that does not hold is **refused**, and it does.

Without `--key` nothing can be verified, so an image a registry does offer a signature for is reported unproven rather than signed. Nothing is ever reported as signed on the strength of an answer that did not arrive.

Usage: lemonfiber plugin provenance [OPTIONS] <PATH>

Arguments:
  <PATH>
          The plugin's source: its directory, or the `plugin.toml` inside it

Options:
      --json
          Print machine-readable output

      --key <PEM>
          A PEM public key to check signatures against. Repeatable

      --dry-run
          Say what would happen, and change nothing

      --force
          Take the stack from a run that claimed it and did not give it back

      --stack-dir <PATH>
          Operate a stack directory of your own instead of the built-in one

      --config-dir <PATH>
          Keep lemonfiber's own configuration under a directory of your own

      --data-dir <PATH>
          Keep lemonfiber's own data under a directory of your own

  -h, --help
          Print help (see a summary with '-h')
```
