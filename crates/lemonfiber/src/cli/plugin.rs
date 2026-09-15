//! The three things a plugin author can ask this binary about.
//!
//! Its own file rather than a line in the request list, because all three are reads of
//! published artefacts rather than anything done to a stack — and because what tells
//! them apart is worth a sentence each, which is more than an index of requests should
//! carry.

use clap::Subcommand;

/// What a plugin author can be told, with nothing running.
///
/// Three reads and no verbs. Nothing here installs, removes or asks anything of a
/// service: these are the documents lemonfiber publishes about what a plugin may
/// claim, where it may contribute, and what shape its manifest takes. Each answers
/// with no network, no catalogue and no stack, and each says which generation it is
/// reporting — an author comparing two answers needs to know whether the difference is
/// their build or their manifest.
#[derive(Debug, Subcommand)]
pub enum PluginCommand {
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
}
