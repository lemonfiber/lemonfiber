# `lemonfiber plugin`

Generated from the command line's own declarations. Run `just reference` to rewrite it.
Part of the [command reference](../commands.md).

## `lemonfiber plugin`

```text
Install, update and remove plugins, and read what one may declare.

Five of the words under this one are documents for somebody writing a plugin, answered with no network, no catalogue and no stack running, each saying which generation it reports — so an author who has to know whether a difference is their build or their manifest can tell. The other four are about this machine: `install` writes down what installing a plugin decides, `installed` reads that back, `update` replaces one version with another as one operation, and `remove` takes one off. Each of the three that acts can be rehearsed with `--dry-run`.

Usage: lemonfiber plugin [OPTIONS] <COMMAND>

Commands:
  capabilities      List the capabilities a service can claim, and what claiming one undertakes
  extension-points  List the places a plugin may extend lemonfiber itself
  schema            Print the schema an editor validates `plugin.toml` against
  claims            Read a plugin's source and say what its claims come to
  provenance        Ask each image's registry whether anybody has said it is theirs
  install           Install a plugin, recording what installing it decided
  installed         Say what is installed, and what each plugin is doing
  remove            Take a plugin off this machine, putting back everything installing it wrote
  update            Replace an installed plugin with another version of it, as one operation
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

## `lemonfiber plugin install`

```text
Install a plugin, recording what installing it decided.

The manifest is held to everything `claims` holds it to before anything is written, and a plugin this build refuses is not installed — a refusal is total, so none of the manifest is acted on and the machine is left as it was.

What is written is the record of what the install settled: the plugin, and for each of its services the image, the digest that pins what runs, the tier it is published on and where inside its container its own configuration directory is mounted. That record is the answer every later step reads — the author's file may be edited or deleted the moment this is done, and a run that went back to it would be answering a question about a document rather than about this machine.

It then says what container lemonfiber writes from that record, which is the question worth asking before a stranger's service is on the machine: the image pinned to its digest, the profile it sits in, the interface its tier publishes it on, and every mount it can ever have. A plugin supplies none of that and there is no field in which it could ask for more of it.

Installing over an installation is refused naming it: that is an update, which puts one set of changes back before it applies another.

`--dry-run` settles everything the real run settles, says the same account of it, and writes nothing.

Usage: lemonfiber plugin install [OPTIONS] <PATH>

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

## `lemonfiber plugin installed`

```text
Say what is installed, and what each plugin is doing.

The one place to look when something about the stack is surprising. For each plugin: where it was installed from and whether anybody reviewed it, when it was installed, where it is published and under what licence, what it claims and fills, what it added, what you chose it to stand in for, what it may change, where it may reach and what it holds — and, for each of its services, what runs and how it is reached.

Read from the record rather than from the manifests, so it answers for a machine whose plugin sources are long gone. A machine with none answers with an empty list and says so.

A record that is there and cannot be read is refused rather than answered as nothing installed: a stranger's service may be running, and *no plugins* is the one wrong answer that would be believed.

Usage: lemonfiber plugin installed [OPTIONS]

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

## `lemonfiber plugin remove`

```text
Take a plugin off this machine, putting back everything installing it wrote.

The id rather than a path: the plugin's own source may be long gone, and what is being removed is a record this machine holds rather than a document somebody still has a copy of.

A removal is the rollback layer's work with a name on it, so it inherits every refusal that layer already makes. A setting edited by hand since the install is drift and is refused rather than overwritten; a change a later change depends on is refused until that one goes back; a change that re-points where data lives says plainly that the data does not move with it.

There is no *disable*. A plugin is installed or it is not — a third state in which one is present but inert is a state nothing else in this product has and one an operator would have to keep in their head.

`--dry-run` says what it would put back and what the machine would be left without, and touches nothing.

Usage: lemonfiber plugin remove [OPTIONS] <PLUGIN>

Arguments:
  <PLUGIN>
          The plugin's id, as `lemonfiber plugin installed` lists it

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

## `lemonfiber plugin update`

```text
Replace an installed plugin with another version of it, as one operation.

The source of the new version, held to everything an install is held to. The version installed comes off the way a removal takes it — every refusal the rollback layer makes, judged before anything moves — and the new one goes on the way an install puts it on: written, started, proved, and held against the stack's own checks as they read before the update began.

The machine is on one version or the other at every moment, never between them. The record of what is installed is written last, once the new version has held; where it does not hold, it goes back and the version it replaced is put back on from its record, and the report says which version the machine is on.

`--dry-run` says what would go back, what the new version would write and prove, and what would stop meanwhile, and touches nothing.

Usage: lemonfiber plugin update [OPTIONS] <PATH>

Arguments:
  <PATH>
          The new version's source: its directory, or the `plugin.toml` inside it

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
