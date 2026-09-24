//! What can go wrong reading a plugin manifest.

use thiserror::Error;

use crate::conforming::Violation;

/// A plugin manifest could not be read.
///
/// The three refusals are separate variants rather than one "invalid manifest"
/// because they have nothing to do with each other: an unreadable *format*, a plugin
/// written against a generation this build does not carry, and a manifest whose
/// declarations the published schema refuses each need a different response from
/// whoever hit it.
#[derive(Debug, Error)]
pub enum Error {
    /// The file is not valid TOML, or does not have the shape of a manifest.
    #[error("the plugin manifest could not be parsed: {0}")]
    Syntax(#[from] toml::de::Error),

    /// The manifest declares a schema generation this build cannot read.
    #[error(
        "the plugin manifest declares schema version {found}, and this build reads {supported:?}"
    )]
    UnsupportedSchema {
        /// The version the manifest declared.
        found: u32,
        /// The versions this build can read.
        supported: Vec<u32>,
    },

    /// The manifest does not conform to the schema this build publishes.
    ///
    /// Every way it does not, because a third-party manifest is far likelier to carry
    /// several faults than a first-party one, and fixing a manifest one error per run
    /// is a guessing game. A field this build has no declaration for, one it needs and
    /// did not get, a value of the wrong kind and a word outside a closed set all
    /// arrive together.
    #[error("the plugin manifest does not conform to the published schema:{}", each(.0))]
    Nonconforming(
        /// What was declared, and what it would have had to be.
        Vec<Violation>,
    ),
}

/// Each refusal on its own indented line, so a list of them reads as a list.
fn each(found: &[Violation]) -> String {
    found.iter().fold(String::new(), |mut listed, one| {
        listed.push_str("\n  ");
        listed.push_str(&one.to_string());
        listed
    })
}

#[cfg(test)]
mod tests;
