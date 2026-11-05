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

use std::path::Path;

use lemonfiber_plugin::seeding::{placeholders, Form};
use lemonfiber_plugin::{Manifest, Shape};
use serde::Serialize;

use super::claimed::Verdict;
use super::installed::Installed;
use super::placing::{configuration, Lands, Write};
use super::seeding::Shipped;

/// What an install puts at one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[schemars(rename = "PluginPuts")]
pub enum Puts {
    /// A directory brought into being, which is where one service keeps its own
    /// configuration.
    Directory,
    /// The project written again, holding the container lemonfiber derives for the
    /// plugin.
    Document,
    /// The key lemonfiber asks one of the plugin's adapters with, minted as it is
    /// written and never shown.
    Key,
    /// A service's own secrets file, readable by its owner alone, its values never shown.
    Secrets,
    /// A file the plugin ships for one of its services, written where none is yet; one
    /// naming a secret is readable by its owner alone and shown with its placeholders.
    Starting,
    /// The credentials an adapter reads its upstream's with, readable by its owner alone,
    /// their values never shown.
    Upstream,
}

/// One change installing a plugin makes to the machine.
///
/// A path and what goes at it, which is the whole of what an install touches. None is an
/// edit to a file the stack already has: a plugin's route is the proxy's routing table
/// and its dashboard entry the panel list's, each applied over its contract.
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
    /// Each placeholder the file as shipped names a secret by, in the order it holds them:
    /// none for anything but a starting file naming a secret, and never a value.
    pub placeholders: Vec<Placeholding>,
}

/// One placeholder a starting file names a secret by, as the plugin ships it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginPlaceholder")]
pub struct Placeholding {
    /// The secret it names, by its id.
    pub secret: String,
    /// The form its value is written in.
    pub form: Form,
}

impl Placeholding {
    /// The placeholder as it is named to an operator.
    #[must_use]
    pub fn named(&self) -> String {
        lemonfiber_plugin::seeding::named(&self.secret, self.form)
    }
}

/// Each placeholder `bytes` name a secret by, in the order they hold them; none where
/// they are not text or a placeholder in them cannot be read.
#[must_use]
pub fn placeholding(bytes: &[u8]) -> Vec<Placeholding> {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| placeholders(text).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|placeholder| Placeholding {
            secret: placeholder.secret.to_owned(),
            form: placeholder.form,
        })
        .collect()
}

/// One starting file whose placeholders an update changes, as the version installed
/// shipped it and as the one coming ships it: the placeholders alone, never what the
/// file on the machine holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginReshipped")]
pub struct Reshipping {
    /// Where the file is, in full.
    pub path: String,
    /// Its placeholders as the version installed shipped it, none where it shipped none.
    pub was: Vec<Placeholding>,
    /// Its placeholders as the version coming ships it, none where it ships none.
    pub becomes: Vec<Placeholding>,
}

/// Each starting file beneath `stack` whose placeholders differ between what the version
/// installed shipped, `was`, and what the one coming ships, `becomes`, in path order.
#[must_use]
pub fn reshipped(stack: &Path, was: &[Shipped], becomes: &[Shipped]) -> Vec<Reshipping> {
    let named = |files: &[Shipped]| -> std::collections::BTreeMap<String, Vec<Placeholding>> {
        files
            .iter()
            .map(|file| {
                (
                    configuration(stack, &file.service)
                        .join(&file.path)
                        .display()
                        .to_string(),
                    placeholding(&file.bytes),
                )
            })
            .collect()
    };
    let (was, becomes) = (named(was), named(becomes));
    let mut paths: Vec<&String> = was.keys().chain(becomes.keys()).collect();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter_map(|path| {
            let before = was.get(path).cloned().unwrap_or_default();
            let after = becomes.get(path).cloned().unwrap_or_default();
            (before != after).then(|| Reshipping {
                path: path.clone(),
                was: before,
                becomes: after,
            })
        })
        .collect()
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

/// One service taking a privileged shape, as the reading states it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginTaking")]
pub struct Taking {
    /// The service taking it.
    pub service: String,
    /// The shape it takes.
    pub shape: Shape,
    /// The kernel capabilities it is given.
    pub grants: Vec<String>,
    /// The devices it is given.
    pub devices: Vec<String>,
    /// Whether it is added to the group owning those devices on the engine's host.
    pub grouped: bool,
    /// What approving it is written as, apart from the offer.
    pub approval: String,
}

/// Every service of the plugin taking a privileged shape, in the order the record
/// keeps them.
#[must_use]
pub fn taking(installed: &Installed) -> Vec<Taking> {
    installed
        .services
        .iter()
        .filter_map(|placed| {
            placed.shape.map(|shape| Taking {
                service: placed.service.clone(),
                shape,
                grants: shape
                    .grants()
                    .iter()
                    .map(|&grant| grant.to_owned())
                    .collect(),
                devices: shape
                    .devices()
                    .iter()
                    .map(|&device| device.to_owned())
                    .collect(),
                grouped: shape.grouped(),
                approval: shape.approval(&placed.service),
            })
        })
        .collect()
}

/// An adapter that writes the files of the service it fronts, through that service's
/// whole configuration directory mounted into it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[schemars(rename = "PluginSharing")]
pub struct Sharing {
    /// The adapter.
    pub adapter: String,
    /// The service whose files it writes.
    pub upstream: String,
    /// Where that service's configuration directory lands inside the adapter.
    pub at: String,
}

/// Every adapter of the plugin sharing the configuration directory of the service it
/// fronts, in the order the record keeps them.
#[must_use]
pub fn sharing(installed: &Installed) -> Vec<Sharing> {
    installed
        .services
        .iter()
        .filter_map(|placed| {
            Some(Sharing {
                adapter: placed.service.clone(),
                upstream: placed.fronts.clone()?,
                at: placed.fields.shares_config.clone()?,
            })
        })
        .collect()
}

/// What the reading lists as approved one by one: each value the recipes would carry
/// elsewhere, each service taking a privileged shape, and each credential granted to the
/// plugin.
#[must_use]
pub fn asked(would: &Installed, taking: &[Taking]) -> Vec<String> {
    let mut asked: Vec<String> = super::approvals(&would.recipes)
        .into_iter()
        .map(str::to_owned)
        .collect();
    asked.extend(taking.iter().map(|one| one.approval.clone()));
    asked.extend(
        would
            .granted
            .iter()
            .map(|grant| grant.approval(&would.plugin)),
    );
    asked
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
                Lands::Directory { .. } => Puts::Directory,
                Lands::Document { .. } => Puts::Document,
                Lands::Key => Puts::Key,
                Lands::Secrets(_) => Puts::Secrets,
                Lands::Starting { .. } => Puts::Starting,
                Lands::Upstream { .. } => Puts::Upstream,
            },
            placeholders: if let Lands::Starting { bytes, .. } = &write.lands {
                placeholding(bytes)
            } else {
                Vec::new()
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
            asks: asking(proof),
            why: proof.why.clone(),
            came_to: None,
        })
        .chain(
            manifest
                .services
                .iter()
                .filter(|service| !service.speaks.is_empty())
                .map(|service| speaking(&service.id)),
        )
        .collect()
}

/// What a proof asks, as a statement says it: the verb and the path of a request, or what
/// a run hands the image's own program.
fn asking(proof: &lemonfiber_plugin::Proof) -> String {
    proof.request.as_ref().map_or_else(
        || {
            proof
                .run
                .as_ref()
                .map(|run| super::recorded::runs(&run.args))
                .unwrap_or_default()
        },
        |request| format!("{} {}", request.method, request.path),
    )
}

/// The proof that the adapter `service` speaks the contracts it declares, not yet asked.
#[must_use]
pub fn speaking(service: &str) -> Proving {
    Proving {
        proof: format!("{service}{SPEAKS}"),
        establishes: "It answers the contracts it says it speaks".to_owned(),
        of: Some(service.to_owned()),
        asks: format!("GET {}", lemonfiber_contract::adapter::about_path()),
        why: "lemonfiber asks it over those contracts, so what it says it speaks has to be \
              what it answers"
            .to_owned(),
        came_to: None,
    }
}

/// A proof kept in the record that runs the image's own program, not yet asked.
#[must_use]
pub fn run_proof(proof: &super::RunProof) -> Proving {
    Proving {
        proof: proof.proof.clone(),
        establishes: proof.title.clone(),
        of: Some(proof.service.clone()),
        asks: super::recorded::runs(&proof.run.args),
        why: proof.why.clone(),
        came_to: None,
    }
}

/// What the proof that an adapter speaks what it declares is named after its service.
pub const SPEAKS: &str = "-speaks";

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
