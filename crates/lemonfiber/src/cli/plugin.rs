//! What can be asked under the word `plugin`.
//!
//! Its own file rather than a line in the request list, because what tells these apart
//! is worth a sentence each, which is more than an index of requests should carry.
//!
//! Five of them are reads of documents this build publishes, answered from the binary
//! with no network, no catalogue and no stack. Two are about one operator's machine —
//! what is installed on it, and what putting something on it decides — so those two
//! are dispatched like every other verb and the five are not.

use std::path::PathBuf;

use clap::Subcommand;

/// What can be asked about a plugin, and what can be done with one.
///
/// Two words and a flattened five. The split is the point of the type rather than a
/// tidying of it: [`Authoring`] holds the documents, which are generated at build
/// time and answer the same on every machine, and the two here are about the machine
/// this is running on. Keeping them apart is what lets the reader that answers a
/// document be handed a value that cannot be a verb — so a word added to one half
/// cannot be quietly answered by the other.
#[derive(Debug, Subcommand)]
pub enum PluginCommand {
    /// The five documents, each a word of its own on the command line.
    #[command(flatten)]
    Authoring(Authoring),
    /// Install a plugin, recording what installing it decided.
    ///
    /// The manifest is held to everything `claims` holds it to before anything is
    /// written, and a plugin this build refuses is not installed — a refusal is
    /// total, so none of the manifest is acted on and the machine is left as it was.
    ///
    /// What is written is the record of what the install settled: the plugin, and
    /// for each of its services the image, the digest that pins what runs, the tier
    /// it is published on and where inside its container its own configuration
    /// directory is mounted. That record is the answer every later step reads —
    /// the author's file may be edited or deleted the moment this is done, and a run
    /// that went back to it would be answering a question about a document rather
    /// than about this machine.
    ///
    /// It then says what container lemonfiber writes from that record, which is the
    /// question worth asking before a stranger's service is on the machine: the image
    /// pinned to its digest, the profile it sits in, the interface its tier publishes
    /// it on, and every mount it can ever have. A plugin supplies none of that and
    /// there is no field in which it could ask for more of it.
    ///
    /// Installing over an installation is refused naming it: that is an update,
    /// which puts one set of changes back before it applies another.
    ///
    /// `--dry-run` settles everything the real run settles, says the same account of
    /// it, and writes nothing.
    Install {
        /// The plugin's source: its directory, or the `plugin.toml` inside it.
        path: PathBuf,
    },
    /// Say what is installed, and what each install decided.
    ///
    /// Read from the record rather than from the manifests, so it answers for a
    /// machine whose plugin sources are long gone. A machine with none answers with
    /// an empty list and says so.
    ///
    /// A record that is there and cannot be read is refused rather than answered as
    /// nothing installed: a stranger's service may be running, and *no plugins* is
    /// the one wrong answer that would be believed.
    Installed,
    /// Take a plugin off this machine, putting back everything installing it wrote.
    ///
    /// The id rather than a path: the plugin's own source may be long gone, and what
    /// is being removed is a record this machine holds rather than a document
    /// somebody still has a copy of.
    ///
    /// A removal is the rollback layer's work with a name on it, so it inherits every
    /// refusal that layer already makes. A setting edited by hand since the install is
    /// drift and is refused rather than overwritten; a change a later change depends on
    /// is refused until that one goes back; a change that re-points where data lives
    /// says plainly that the data does not move with it.
    ///
    /// There is no *disable*. A plugin is installed or it is not — a third state in
    /// which one is present but inert is a state nothing else in this product has and
    /// one an operator would have to keep in their head.
    ///
    /// `--dry-run` says what it would put back and what the machine would be left
    /// without, and touches nothing.
    Remove {
        /// The plugin's id, as `lemonfiber plugin installed` lists it.
        plugin: String,
    },
}

/// What a plugin author can be told, with nothing running.
///
/// Five reads and no verbs. Nothing here installs, removes or changes anything:
/// three are the documents lemonfiber publishes about what a plugin may claim, where
/// it may contribute and what shape its manifest takes, the fourth reads a manifest
/// on a path and says what this build makes of it, and the fifth asks each image's
/// registry what it holds beside that image.
///
/// The first four answer with no network, no catalogue and no stack, and each says
/// which generation it is reporting — an author comparing two answers needs to know
/// whether the difference is their build or their manifest. The fifth is the one that
/// reaches out, which is why it is its own request rather than part of another.
#[derive(Debug, Subcommand)]
pub enum Authoring {
    /// List the capabilities a service can claim, and what claiming one undertakes.
    ///
    /// A capability is a named, contracted thing a service can do, so that wiring can
    /// ask for one rather than name a service. Each carries the prose a claimant is
    /// held to, which bundled services already declare it, and the probes a claim has
    /// to bind — the vocabulary says what must be shown, and the claimant says where to
    /// ask.
    ///
    /// A name here is the only kind a plugin may claim without namespacing it. A
    /// plugin's own capability is written `<plugin-id>:<name>` and is inert until
    /// something asks for it.
    Capabilities,
    /// List the places a plugin may extend lemonfiber itself.
    ///
    /// A point is a register lemonfiber already runs, and the point names where a
    /// plugin may put another row in it — never a hook and never code. Each says what
    /// a row carries, what is already standing in that register, and the capability a
    /// manifest has to ask for in order to contribute there.
    ExtensionPoints,
    /// Print the schema an editor validates `plugin.toml` against.
    ///
    /// Generated from the types lemonfiber reads a manifest with, so it describes the
    /// reader rather than claiming something about it. Always machine-readable: it is
    /// a document for an editor rather than a listing for a person.
    Schema,
    /// Read a plugin's source and say what its claims come to.
    ///
    /// The three documents above say what may be written; this says what one manifest
    /// wrote. Everything it declares is held to the published schema, to the published
    /// vocabulary and to the published points in one pass — the fields, the kinds, the
    /// digest that fixes what runs, the paths, and the capabilities it asks of this
    /// build — every probe a claim binds is run against the recording it names, and each
    /// capability is reported as demonstrated, unproven or refuted rather than as
    /// claimed.
    ///
    /// It then says what asking for each capability would come to on the stack this
    /// build pins: filled by one service, contested between several — which lemonfiber
    /// refuses to settle by install order — or inert, which is what a capability of the
    /// plugin's own is until something asks for it.
    ///
    /// A refusal, or a claim its own recordings refute, exits non-zero. Nothing is
    /// installed, nothing is written, and no service is asked anything.
    Claims {
        /// The plugin's source: its directory, or the `plugin.toml` inside it.
        path: PathBuf,
    },
    /// Ask each image's registry whether anybody has said it is theirs.
    ///
    /// The one read here that reaches the network, and the only one: it asks the
    /// registry the manifest pins an image in whether it offers a signature for that
    /// exact digest, and says what this build makes of the answer.
    ///
    /// Three answers and never two. An image whose signature verifies against a key
    /// you hold is **signed**. One nobody signed is **unproven** — a publisher who
    /// signed nothing has made no claim, which is a different fact from a claim that
    /// did not check out, and it does not stop an install. One carrying a signature
    /// that does not hold is **refused**, and it does.
    ///
    /// Without `--key` nothing can be verified, so an image a registry does offer a
    /// signature for is reported unproven rather than signed. Nothing is ever
    /// reported as signed on the strength of an answer that did not arrive.
    Provenance {
        /// The plugin's source: its directory, or the `plugin.toml` inside it.
        path: PathBuf,
        /// A PEM public key to check signatures against. Repeatable.
        #[arg(long = "key", value_name = "PEM")]
        keys: Vec<PathBuf>,
    },
}
