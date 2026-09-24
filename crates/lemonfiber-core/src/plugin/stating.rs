//! What installing a plugin says it will do, in the terms an operator decides on.
//!
//! A rehearsal is worth having only if it is the account the install then follows, so
//! nothing here is built for the rehearsal: it is the same derivation, read off the
//! same manifest and the same list of writes, on the run that reports and on the run
//! that acts. The tense is the surface's business; the facts are one set.
//!
//! **What is stated is what lands on the machine, not what is journalled.** The two
//! lists are nearly the same and differ in the one place that matters to a person
//! being asked to agree: a directory that is already there is not written and not
//! recorded, and a document left behind by an install that did not finish is
//! overwritten without being recorded — because recording it as *made* would have a
//! reversal remove a file this run did not create. The second of those is a change to
//! the machine and belongs here; the record's job is different, and it is the record
//! that gets to be narrower.
//!
//! Nothing here touches a disk. Every answer is a function of the manifest and of the
//! list [`super::placing::writes`] derives, so what an install would say can be put in
//! front of a test without a stack directory under it.

use lemonfiber_plugin::Manifest;
use serde::Serialize;

use super::claimed::Verdict;
use super::placing::{Lands, Write};

/// What an install puts at one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "PluginPuts")]
pub enum Puts {
    /// A directory brought into being, which is where one service keeps its own
    /// configuration.
    Directory,
    /// A document written, which is the container lemonfiber derives for the plugin.
    Document,
    /// A region written into one of the stack's own files, marked out as the plugin's:
    /// its route through the proxy, or its entry on the dashboard.
    Region,
}

/// One change installing a plugin makes to the machine.
///
/// A path and what goes at it, which is the whole of what an install touches. Two of
/// them can be edits to a file the stack already has — the proxy's and the
/// dashboard's — and those say so, as a region, so an operator reading the account
/// knows which of their files the install writes into.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginChange")]
pub struct Changing {
    /// Where it lands, in full.
    ///
    /// In full rather than relative to the stack, because *what is this about to do
    /// to my machine* is answered by a path somebody can go and look at — and a
    /// relative one is right about a directory the reader has to work out for
    /// themselves.
    pub path: String,
    /// What lands there.
    pub puts: Puts,
}

/// One proof that has to hold before a plugin is reported installed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginProving")]
pub struct Proving {
    /// The proof's id, which its verdict is reported against.
    pub proof: String,
    /// What it establishes, in one line.
    pub establishes: String,
    /// Which of the plugin's services it asks, where the manifest settles that.
    ///
    /// Nothing where it does not, which is a manifest the reader has already refused
    /// — carried as an absence rather than as a guess, so that a report built from a
    /// manifest nobody held to the reader says *this was not settled* instead of
    /// naming whichever service came first.
    pub of: Option<String>,
    /// What it asks, as the method and the path it is asked at.
    pub asks: String,
    /// Why it is worth asserting.
    pub why: String,
    /// What asking it came to, or nothing where it was not asked.
    ///
    /// Absent on a rehearsal, which asks nothing. That is a different fact from a
    /// proof that was asked and established nothing, and the two must not read alike:
    /// one is an account of what would happen, the other is a service that did not
    /// answer.
    pub came_to: Option<Verdict>,
}

/// One bundled thing a plugin declares it will change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "PluginOverriding")]
pub struct Overriding {
    /// Which bundled setting it changes.
    pub setting: String,
    /// What changing it is for.
    pub why: String,
}

/// Every change installing this plugin makes, in the order it makes them.
///
/// Read off the very list the writes are carried out from, so the account and the act
/// cannot come apart. A second walk over the record would be a second derivation, free
/// to state a path the install does not write.
#[must_use]
pub fn changes(planned: &[Write]) -> Vec<Changing> {
    planned
        .iter()
        .map(|write| Changing {
            path: write.path.display().to_string(),
            puts: match write.lands {
                Lands::Directory => Puts::Directory,
                Lands::Document(_) => Puts::Document,
                Lands::Region { .. } => Puts::Region,
            },
        })
        .collect()
}

/// Every proof the install would run, in the order the manifest declares them.
///
/// Stated rather than run. What a rehearsal owes an operator is *which questions will
/// be asked of what*, before there is anything to ask them of; the verdicts are the
/// install's own business and are reached where the asking happens.
#[must_use]
pub fn proofs(manifest: &Manifest) -> Vec<Proving> {
    manifest
        .proofs
        .iter()
        .map(|proof| Proving {
            proof: proof.id.clone(),
            establishes: proof.title.clone(),
            of: manifest
                .asks(proof.service.as_deref())
                .map(|service| service.id.clone()),
            asks: format!("{} {}", proof.request.method, proof.request.path),
            why: proof.why.clone(),
            came_to: None,
        })
        .collect()
}

/// Every bundled thing the plugin declares it will change.
///
/// **This is the full extent rather than a sample of it, and that is a property of the
/// reader rather than a promise made here.** A manifest may change a bundled setting
/// only through a recipe, and a recipe reaching a setting no `[[override]]` names is
/// refused before anything is installed — so what is declared is what can happen, and
/// an empty list means a plugin that changes nothing of the stack's.
#[must_use]
pub fn overrides(manifest: &Manifest) -> Vec<Overriding> {
    manifest
        .overrides
        .iter()
        .map(|one| Overriding {
            setting: one.id.clone(),
            why: one.why.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests;
