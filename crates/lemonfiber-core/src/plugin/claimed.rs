//! What a plugin's source says it can do, and what this build makes of it.
//!
//! A manifest on a path, read with no network, no catalogue and no stack running. What
//! comes back is the three answers an author and an operator both want and neither had:
//! whether the two published vocabularies refuse anything it declares, whether each
//! claimed capability is actually demonstrated by the recordings it binds, and what
//! asking for that capability would come to on the stack this build pins.
//!
//! **A claim is demonstrated, not asserted**, and the three verdicts are three. A probe
//! whose recording refuses it makes the claim false and the plugin uninstallable; one
//! whose recording cannot be read at all establishes nothing, and unproven is never
//! counted as shown.

mod asserting;

use std::path::{Path, PathBuf};

use lemonfiber_plugin::{Claim, ClaimProbe, Manifest, Service};

use crate::doctor::BUNDLED_CHECKS;
use crate::filling::{fills, Claimant, Filling, Shown};
use crate::plugin::{capabilities, Ungenerated};
pub use asserting::{Asserted, Assertion};

/// The one file a plugin is described by.
const MANIFEST: &str = "plugin.toml";

/// Why a plugin's source could not be read at all.
///
/// Distinct from anything wrong with what it declares: nothing here is the author's
/// manifest being refused, and telling the two apart is the difference between "fix
/// line 40" and "you are pointing at the wrong directory".
#[derive(Debug, thiserror::Error)]
pub enum Unreadable {
    /// Nothing at that path holds a plugin manifest.
    #[error("no {MANIFEST} at {}", .0.display())]
    NoManifest(PathBuf),

    /// The file is there and could not be read from the disk.
    #[error("{MANIFEST} could not be read: {0}")]
    Unopenable(#[from] std::io::Error),

    /// The manifest is there and this build cannot read what it says.
    #[error(transparent)]
    Refused(#[from] lemonfiber_plugin::Error),

    /// The stack this build pins could not be read, so nothing could be said about
    /// what a claim would fill.
    #[error(transparent)]
    Stack(#[from] Ungenerated),
}

/// What the verdicts in a report were reached against.
///
/// One value, because one is all this build can produce: nothing here asks a service
/// anything. It is a field rather than a sentence for the reason a verdict is one — a
/// reader handed `demonstrated` has nothing else in the document to tell a recording
/// that answered from a service that did, and the weaker of those two claims must not
/// be readable as the stronger. The prose says it on the page; this says it to
/// whatever consumes the report: an author's own CI, a catalogue, anything counting
/// passes. Naming the axis now is what made the second kind of evidence a change the
/// compiler walked somebody through rather than one they had to remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
#[schemars(rename = "PluginEvidence")]
pub enum Evidence {
    /// The recordings the plugin ships. No service was asked anything.
    Recordings,
    /// The service itself, running on this machine and asked.
    ///
    /// The stronger of the two and the one only an install can reach: an author has
    /// no instance to ask and the catalogue has none either, which is the whole
    /// reason recordings exist. A verdict reached here says the service did the
    /// thing on the machine it is installed on, and it must not be readable as the
    /// weaker claim any more than the weaker may be read as this.
    Service,
}

/// What one assertion came to, whatever answered it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
#[serde(tag = "outcome", rename_all = "lowercase")]
#[schemars(rename = "PluginVerdict")]
pub enum Verdict {
    /// The recording answers what the binding declares.
    Passed,
    /// It does not, in every way it does not.
    Failed {
        /// Every way the recorded answer is not the declared one.
        faults: Vec<String>,
    },
    /// It could not be run, and so established nothing either way.
    Unproven {
        /// What stopped it being run.
        why: String,
    },
}

/// One probe a claim binds, and what running it against its recording came to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Ran {
    /// The probe the vocabulary declares, by name.
    pub probe: String,
    /// What the recording said about it.
    pub verdict: Verdict,
}

/// One capability a plugin's service declares, and what this build makes of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Claiming {
    /// The capability's name.
    pub name: String,
    /// The service declaring it.
    pub service: String,
    /// Whether it is this plugin's own rather than one from the core vocabulary.
    pub own: bool,
    /// How far the claim has been shown.
    pub shown: Shown,
    /// Every probe the claim binds, in the order the vocabulary declares them.
    pub probes: Vec<Ran>,
    /// What asking for it would come to, or nothing where nothing can ask.
    ///
    /// A namespaced capability has no answer here rather than an empty one: nothing
    /// consumes it, which is what *inert* means, and reporting it as unfilled would be
    /// describing a gap where there is a design.
    pub filling: Option<Filling>,
}

/// One row a plugin adds to a register lemonfiber already runs.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Contributed {
    /// The point it is made at.
    pub at: String,
    /// The identity it holds, namespaced with the plugin's id.
    pub id: String,
    /// What it says, in one line.
    pub says: String,
    /// The check a remedy is for, where this is one.
    #[serde(rename = "for")]
    pub about: Option<String>,
}

/// Everything a plugin's source says, and what this build makes of it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Claimed {
    /// The plugin's id.
    pub id: String,
    /// Its human-facing name.
    pub name: String,
    /// The author's own version of it.
    pub version: String,
    /// The capability vocabulary generation it was held to.
    ///
    /// Carried for the reason every other read under this word carries one: an author
    /// comparing what they were refused for with what they read needs to know whether
    /// the difference is their manifest or this build.
    pub vocabulary_version: u32,
    /// The extension points generation it was held to.
    pub extension_points_version: u32,
    /// What every verdict in this report was reached against.
    pub against: Evidence,
    /// Everything the two published vocabularies refuse, in one pass.
    pub refusals: Vec<lemonfiber_plugin::Violation>,
    /// What its services declare they can do.
    pub capabilities: Vec<Claiming>,
    /// What must hold before it is installed, against the recordings it ships.
    pub proofs: Vec<Asserted>,
    /// The checks it contributes, against the recordings they name.
    ///
    /// Reported and never counted against the install. A contributed check reports on a
    /// running stack rather than gating one, so a check that its own recording refuses
    /// is a check doing its job on a machine in the state it was recorded in — which is
    /// a different fact from a claim its recordings refute.
    pub checks: Vec<Asserted>,
    /// What it adds to lemonfiber's own registers.
    pub contributions: Vec<Contributed>,
    /// Whether this plugin would be installed as it stands.
    ///
    /// Carried rather than left for a reader to work out. It is one rule over two of
    /// the fields above, and a consumer deciding it for itself would be a second copy
    /// of that rule — free to disagree with the exit status this same answer produces.
    pub installable: bool,
}

/// The manifest at this path, read and refused the same way `claimed` reads one.
///
/// Its own entry point because two reads want the file and only one of them wants a
/// verdict about the claims in it. A second copy of *where a plugin's manifest is*
/// would be free to disagree about whether a directory or the file inside it was
/// meant, which is the one thing both callers have to agree on.
///
/// # Errors
///
/// [`Unreadable`] where there is no manifest at the path, or where this build cannot
/// read the one that is there.
pub fn read(path: &Path) -> Result<Manifest, Unreadable> {
    sourced(path).map(|(_, manifest)| manifest)
}

/// The plugin's root and the manifest inside it, or why neither could be had.
///
/// The one answer to *where a plugin's manifest is*, because the two reads above
/// would otherwise each carry their own and be free to disagree about whether a
/// directory or the file inside it was meant.
fn sourced(path: &Path) -> Result<(PathBuf, Manifest), Unreadable> {
    let (root, at) = source(path);
    if !at.is_file() {
        return Err(Unreadable::NoManifest(root));
    }
    let manifest = Manifest::from_toml(&std::fs::read_to_string(&at)?)?;
    Ok((root, manifest))
}

/// What the plugin whose source is at this path claims, and what it comes to.
///
/// # Errors
///
/// [`Unreadable`] where there is no manifest at the path, where this build cannot read
/// the one that is there, or where the stack this build pins cannot be read.
pub fn claimed(path: &Path) -> Result<Claimed, Unreadable> {
    let (root, manifest) = sourced(path)?;
    let published = capabilities()?;

    let mut refusals = lemonfiber_plugin::refusals(&manifest, BUNDLED_CHECKS);
    let mut capabilities = Vec::new();
    for service in &manifest.services {
        for name in &service.provides {
            let claiming = declaring(&manifest, service, name, &root, &mut refusals);
            capabilities.push(claiming);
        }
    }
    for claiming in &mut capabilities {
        claiming.filling = filled(claiming, &published, &manifest.plugin.id);
    }
    let proofs = asserting::proved(&manifest, &root, &mut refusals);
    let checks = asserting::checked(&manifest, &root, &mut refusals);

    Ok(Claimed {
        id: manifest.plugin.id.clone(),
        name: manifest.plugin.name.clone(),
        version: manifest.plugin.version.clone(),
        vocabulary_version: lemonfiber_plugin::vocabulary::VOCABULARY_VERSION,
        extension_points_version: lemonfiber_plugin::extension::EXTENSION_POINTS_VERSION,
        against: Evidence::Recordings,
        installable: installs(&refusals, &capabilities, &proofs),
        refusals,
        capabilities,
        proofs,
        checks,
        contributions: manifest.contributions.iter().map(contributed).collect(),
    })
}

/// Whether what was read would be installed.
///
/// A refusal is the manifest contradicting what this build publishes, a refuted claim is
/// a service that does not do what it says, and a proof its own recording refuses is the
/// plugin failing its own condition for being installed at all — all three stop an
/// install. An unproven one does not, in any of the three: nothing has been established
/// about the service, so the capability goes unfilled and the plugin is installed
/// without filling it, which is a different fact from the service being broken.
///
/// A contributed check is not here. It reports on a running stack rather than gating
/// one, so a check its recording refuses is a check finding the thing it exists to
/// find.
fn installs(
    refusals: &[lemonfiber_plugin::Violation],
    capabilities: &[Claiming],
    proofs: &[Asserted],
) -> bool {
    refusals.is_empty()
        && !capabilities
            .iter()
            .any(|claiming| claiming.shown == Shown::Refuted)
        && !proofs
            .iter()
            .any(|proof| matches!(proof.verdict, Verdict::Failed { .. }))
}

/// The plugin's own directory and the manifest inside it.
///
/// A path to either is accepted, because both are what somebody has to hand: the
/// directory they cloned, or the file their editor is open on.
fn source(path: &Path) -> (PathBuf, PathBuf) {
    if path.file_name().is_some_and(|named| named == MANIFEST) {
        let root = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        return (root, path.to_path_buf());
    }
    (path.to_path_buf(), path.join(MANIFEST))
}

/// One declared capability, with its probes run against the recordings they bind.
///
/// A namespaced capability has no probes to run and never will: there is no published
/// contract for it to satisfy, which is what makes it inert. It is reported as claimed
/// rather than as unproven, because nothing was left unasked.
fn declaring(
    manifest: &Manifest,
    service: &Service,
    name: &str,
    root: &Path,
    refusals: &mut Vec<lemonfiber_plugin::Violation>,
) -> Claiming {
    let own = !lemonfiber_plugin::vocabulary::is_core_name(name);
    let claim = manifest
        .claims
        .iter()
        .find(|claim| claim.capability == name);
    let probes = claim.map_or_else(Vec::new, |claim| ran(claim, service, root, refusals));

    Claiming {
        name: name.to_owned(),
        service: service.id.clone(),
        own,
        shown: shown(own, &probes),
        probes,
        filling: None,
    }
}

/// Every probe a claim binds, run against the recording it names.
fn ran(
    claim: &Claim,
    service: &Service,
    root: &Path,
    refusals: &mut Vec<lemonfiber_plugin::Violation>,
) -> Vec<Ran> {
    claim
        .probes
        .iter()
        .map(|binding| Ran {
            probe: binding.id.clone(),
            verdict: against(binding, service, root, refusals),
        })
        .collect()
}

/// One binding, against the recording it names.
fn against(
    binding: &ClaimProbe,
    service: &Service,
    root: &Path,
    refusals: &mut Vec<lemonfiber_plugin::Violation>,
) -> Verdict {
    asserting::recorded_answer(
        Some(binding.fixture.as_str()),
        &binding.request,
        &binding.expect,
        Some(service),
        root,
        refusals,
    )
}

/// How far a claim has been shown, from what its probes came to.
///
/// One refuted probe refutes the claim: the contract is the whole of what the probes
/// ask together, so a service that answers three of four has not satisfied it. One
/// unproven probe leaves the claim unproven for the same reason — what was not asked
/// cannot have been answered.
fn shown(own: bool, probes: &[Ran]) -> Shown {
    if own {
        return Shown::Claimed;
    }
    if probes.is_empty() {
        return Shown::Unproven;
    }
    if probes
        .iter()
        .any(|ran| matches!(ran.verdict, Verdict::Failed { .. }))
    {
        return Shown::Refuted;
    }
    if probes
        .iter()
        .any(|ran| matches!(ran.verdict, Verdict::Unproven { .. }))
    {
        return Shown::Unproven;
    }
    Shown::Demonstrated
}

/// What asking for a claimed capability would come to, on the stack this build pins.
///
/// The bundled claimants are read out of the published vocabulary rather than the stack
/// itself, because that artefact is the one a plugin author is holding — so what they
/// are told here and what they read there cannot disagree.
fn filled(claiming: &Claiming, published: &super::Capabilities, plugin: &str) -> Option<Filling> {
    if claiming.own {
        return None;
    }
    let mut claimants: Vec<Claimant> = published
        .capabilities
        .iter()
        .find(|capability| capability.name == claiming.name)?
        .declared_by
        .iter()
        .map(|service| Claimant {
            service: service.clone(),
            plugin: None,
            shown: Shown::Claimed,
        })
        .collect();
    claimants.push(Claimant {
        service: claiming.service.clone(),
        plugin: Some(plugin.to_owned()),
        shown: claiming.shown.clone(),
    });
    Some(fills(&claimants))
}

/// One contributed row, in the terms a listing shows it.
fn contributed(entry: &lemonfiber_plugin::Contribution) -> Contributed {
    Contributed {
        at: entry.at.clone(),
        id: entry.id.clone(),
        says: entry
            .title
            .clone()
            .or_else(|| entry.action.clone())
            .unwrap_or_default(),
        about: entry.about.clone(),
    }
}

#[cfg(test)]
mod tests;
