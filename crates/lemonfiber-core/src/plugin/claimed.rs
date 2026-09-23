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
mod tests {
    use std::path::{Path, PathBuf};

    use super::{claimed, Shown, Unreadable, Verdict, BUNDLED_CHECKS};

    /// A plugin's source, as one lands on a reviewer's disk.
    const MANIFEST: &str = r#"
schema_version = 1

[plugin]
id          = "kavita"
name        = "Kavita"
version     = "1.0.0"
description = "Reads comics in a browser"
without_it  = "Comics stay folders of images"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "kavita"
name        = "Kavita"
image       = "example.invalid/kavita"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.0.0"
port        = 5000
bind        = "lan"
criticality = "enhancing"
provides    = ["media.serve", "kavita:opds"]

[[claim]]
capability = "media.serve"

[[claim.probe]]
id      = "guarded"
request = { method = "GET", path = "/api/series" }
expect  = { status = 401 }
fixture = "fixtures/guarded.json"

[[claim.probe]]
id      = "catalogue"
request = { method = "GET", path = "/api/series" }
expect  = { status = 200, json_has_keys = ["content"] }
fixture = "fixtures/catalogue.json"
"#;

    /// The image the manifest above pins, as a recording names it.
    const PINNED: &str = "example.invalid/kavita@sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945";

    fn guarded() -> String {
        recording(PINNED, "GET", "/api/series", r#"{"status": 401}"#)
    }

    fn catalogue() -> String {
        recording(
            PINNED,
            "GET",
            "/api/series",
            r#"{"status": 200, "json": {"content": []}}"#,
        )
    }

    fn recording(from: &str, method: &str, path: &str, response: &str) -> String {
        format!(
            r#"{{"recorded_from": "{from}", "note": "n",
                "request": {{"method": "{method}", "path": "{path}"}},
                "response": {response}}}"#
        )
    }

    /// A plugin source written to a scratch directory, with whatever recordings.
    fn source(named: &str, manifest: &str, fixtures: &[(&str, String)]) -> PathBuf {
        let at = std::env::temp_dir().join(format!("lemonfiber-claimed-{named}"));
        let _ = std::fs::remove_dir_all(&at);
        let _ = std::fs::create_dir_all(at.join("fixtures"));
        let _ = std::fs::write(at.join("plugin.toml"), manifest);
        for (name, body) in fixtures {
            let _ = std::fs::write(at.join(name), body);
        }
        at
    }

    /// A whole plugin, with both recordings answering what the claim declares.
    fn whole(named: &str) -> PathBuf {
        source(
            named,
            MANIFEST,
            &[
                ("fixtures/guarded.json", guarded()),
                ("fixtures/catalogue.json", catalogue()),
            ],
        )
    }

    /// The state of the one core capability, and what its probes came to.
    fn core(at: &Path) -> Option<(Shown, Vec<Verdict>)> {
        let read = claimed(at).ok()?;
        let claiming = read
            .capabilities
            .into_iter()
            .find(|claiming| !claiming.own)?;
        Some((
            claiming.shown,
            claiming.probes.into_iter().map(|ran| ran.verdict).collect(),
        ))
    }

    #[test]
    fn a_claim_whose_recordings_answer_it_is_demonstrated() {
        let at = whole("demonstrated");
        assert_eq!(
            core(&at),
            Some((Shown::Demonstrated, vec![Verdict::Passed, Verdict::Passed]))
        );
        assert!(claimed(&at).is_ok_and(|read| read.installable));
    }

    /// The whole contract is what the probes ask together, so one refused probe refutes
    /// the claim — and a refuted claim is a plugin that does not install.
    #[test]
    fn a_recording_that_refuses_a_probe_refutes_the_claim_and_stops_the_install() {
        let at = source(
            "refuted",
            MANIFEST,
            &[
                (
                    "fixtures/guarded.json",
                    recording(PINNED, "GET", "/api/series", r#"{"status": 200}"#),
                ),
                ("fixtures/catalogue.json", catalogue()),
            ],
        );
        let read = core(&at);
        assert_eq!(
            read.as_ref().map(|(shown, _)| shown.clone()),
            Some(Shown::Refuted)
        );
        assert!(
            read.as_ref().is_some_and(|(_, verdicts)| matches!(
                verdicts.first(),
                Some(Verdict::Failed { faults }) if !faults.is_empty()
            )),
            "got: {read:?}"
        );
        assert!(claimed(&at).is_ok_and(|read| !read.installable));
    }

    /// A recording that is not there establishes nothing, which is not the same as the
    /// service being broken — so the claim is unproven and the plugin still installs.
    #[test]
    fn a_recording_that_is_absent_leaves_the_claim_unproven_rather_than_false() {
        let at = source("absent", MANIFEST, &[("fixtures/guarded.json", guarded())]);
        let read = core(&at);
        assert_eq!(
            read.as_ref().map(|(shown, _)| shown.clone()),
            Some(Shown::Unproven)
        );
        assert!(
            read.as_ref().is_some_and(|(_, verdicts)| matches!(
                verdicts.get(1),
                Some(Verdict::Unproven { why }) if why.contains("catalogue")
            )),
            "got: {read:?}"
        );
        assert!(claimed(&at).is_ok_and(|read| read.installable));
    }

    /// The two halves have drifted, and nothing about the service has been established
    /// either way. Reporting it as a failure would say the service is broken when the
    /// recording is.
    #[test]
    fn a_recording_of_another_call_is_unproven_and_names_both_calls() {
        let at = source(
            "drifted",
            MANIFEST,
            &[
                (
                    "fixtures/guarded.json",
                    recording(PINNED, "GET", "/api/v2/series", r#"{"status": 401}"#),
                ),
                ("fixtures/catalogue.json", catalogue()),
            ],
        );
        let read = core(&at);
        assert_eq!(
            read.as_ref().map(|(shown, _)| shown.clone()),
            Some(Shown::Unproven)
        );
        assert!(
            read.as_ref().is_some_and(|(_, verdicts)| matches!(
                verdicts.first(),
                Some(Verdict::Unproven { why })
                    if why.contains("/api/v2/series") && why.contains("/api/series")
            )),
            "got: {read:?}"
        );
    }

    /// A recording of another build passes while describing software nobody is
    /// installing, which is exactly what a pin moved without re-recording leaves.
    #[test]
    fn a_recording_from_another_image_is_refused_naming_both() {
        let elsewhere = recording(
            "example.invalid/kavita@sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "GET",
            "/api/series",
            r#"{"status": 401}"#,
        );
        let at = source(
            "elsewhere",
            MANIFEST,
            &[
                ("fixtures/guarded.json", elsewhere),
                ("fixtures/catalogue.json", catalogue()),
            ],
        );
        let read = claimed(&at).ok();
        let said: Vec<String> = read
            .as_ref()
            .map(|read| read.refusals.iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        assert!(
            said.iter()
                .any(|one| one.contains("fixtures/guarded.json") && one.contains("0000000")),
            "got: {said:?}"
        );
        assert_eq!(
            read.as_ref().map(|read| read.installable),
            Some(false),
            "a refusal stops the install"
        );
        // Refused rather than run against: the recording answers what the binding
        // declares, and a verdict off it would be about the wrong image.
        assert_eq!(
            core(&at).map(|(shown, verdicts)| (shown, verdicts.first().cloned())),
            Some((
                Shown::Unproven,
                Some(Verdict::Unproven {
                    why: said.first().cloned().unwrap_or_default()
                })
            ))
        );
    }

    /// The condition the plugin sets on its own install, as an author writes one.
    const PROVING: &str = r#"
[[proof]]
id      = "reachable"
title   = "It answers on the path it says it is reached at"
request = { method = "GET", path = "/api/health" }
expect  = { status = 200 }
fixture = "fixtures/health.json"
why     = "A reader nothing can reach is one nobody can open."
"#;

    /// A plugin carrying that condition, with whatever its recording answered.
    fn proving(named: &str, status: u16) -> PathBuf {
        source(
            named,
            &format!("{MANIFEST}{PROVING}"),
            &[
                ("fixtures/guarded.json", guarded()),
                ("fixtures/catalogue.json", catalogue()),
                (
                    "fixtures/health.json",
                    recording(
                        PINNED,
                        "GET",
                        "/api/health",
                        &format!("{{\"status\": {status}}}"),
                    ),
                ),
            ],
        )
    }

    /// A proof is the plugin's own condition for being installed, and a recording that
    /// refuses one stops the install the way a refuted claim does.
    ///
    /// Read through the whole report rather than through the runner, because what a
    /// proof decides is the install — one rule over the proofs and the claims together,
    /// which nothing asking about a single verdict can see. The claim here answers
    /// either way, so the only thing moving is the proof.
    #[test]
    fn a_proof_its_recording_refuses_stops_the_install_and_one_it_answers_does_not() {
        let held = claimed(&proving("proved", 200)).ok();
        assert_eq!(
            held.as_ref().map(|read| (
                read.proofs.iter().map(|one| one.id.clone()).collect(),
                read.proofs.iter().map(|one| one.verdict.clone()).collect(),
                read.installable
            )),
            Some((vec!["reachable".to_owned()], vec![Verdict::Passed], true))
        );

        let refused = claimed(&proving("unproved", 503)).ok();
        assert_eq!(
            refused.as_ref().map(|read| read.installable),
            Some(false),
            "a proof its own recording refuses is a condition the plugin set and failed"
        );
        assert!(
            refused.is_some_and(|read| read.proofs.iter().any(|one| matches!(
                &one.verdict,
                Verdict::Failed { faults } if !faults.is_empty()
            ))),
            "and the verdict says what the answer was"
        );
    }

    /// The claim a bundled service also makes is contested, and every claimant is
    /// named — which is what an operator resolves it by choosing from.
    #[test]
    fn a_core_capability_the_bundled_stack_also_claims_is_contested_and_names_everyone() {
        let at = whole("contested");
        let filling = claimed(&at).ok().and_then(|read| {
            read.capabilities
                .into_iter()
                .find(|claiming| !claiming.own)
                .and_then(|claiming| claiming.filling)
        });
        // Read off the rendering rather than destructured, because an arm for the
        // answers this is not would be a line no run enters.
        let said = format!("{filling:?}");
        assert!(said.contains("Contested"), "got: {said}");
        assert!(
            said.contains("jellyfin"),
            "the bundled claimants are named: {said}"
        );
        assert!(
            said.contains("kavita (plugin kavita)"),
            "and so is the plugin's own: {said}"
        );
    }

    /// Nothing asks for a namespaced capability, so there is no answer to give about
    /// what fills it — which is a different thing from nothing filling it.
    #[test]
    fn a_capability_of_the_plugins_own_is_inert_rather_than_unfilled() {
        let at = whole("inert");
        let own = claimed(&at).ok().and_then(|read| {
            read.capabilities
                .into_iter()
                .find(|claiming| claiming.own)
                .map(|claiming| (claiming.name, claiming.filling, claiming.shown))
        });
        assert_eq!(own, Some(("kavita:opds".to_owned(), None, Shown::Claimed)));
    }

    #[test]
    fn a_core_name_with_no_claim_is_unproven_as_well_as_refused() {
        let (before, _) = MANIFEST.split_once("[[claim]]").unwrap_or_default();
        let at = source("unclaimed", before, &[]);
        let read = claimed(&at).ok();
        assert_eq!(
            read.as_ref().and_then(|read| {
                read.capabilities
                    .iter()
                    .find(|claiming| !claiming.own)
                    .map(|claiming| claiming.shown.clone())
            }),
            Some(Shown::Unproven)
        );
        assert_eq!(
            read.as_ref().map(|read| read.refusals.is_empty()),
            Some(false),
            "and the vocabulary says why"
        );
    }

    /// A core-looking name nothing publishes has no answer about what fills it, and
    /// that is not the answer a namespaced one gets: one is a capability that does not
    /// exist and the other is inert by design.
    #[test]
    fn a_core_name_the_vocabulary_does_not_carry_fills_nothing_and_is_not_inert() {
        let at = source(
            "unpublished",
            &MANIFEST.replace("media.serve", "media.stream"),
            &[("fixtures/guarded.json", guarded())],
        );
        let read = claimed(&at).ok();
        assert_eq!(
            read.as_ref().and_then(|read| {
                read.capabilities
                    .iter()
                    .find(|claiming| !claiming.own)
                    .map(|claiming| (claiming.name.clone(), claiming.filling.clone()))
            }),
            Some(("media.stream".to_owned(), None))
        );
        assert_eq!(
            read.as_ref().map(|read| read.installable),
            Some(false),
            "and the manifest is refused"
        );
    }

    #[test]
    fn a_path_with_no_manifest_says_that_rather_than_anything_about_a_plugin() {
        let at = std::env::temp_dir().join("lemonfiber-claimed-nothing-here");
        let _ = std::fs::remove_dir_all(&at);
        assert!(matches!(claimed(&at), Err(Unreadable::NoManifest(_))));
    }

    /// What a manifest contributes comes back in the terms a listing shows it.
    ///
    /// Both halves of the line a row is shown by: a check says its title, a remedy has
    /// none and says its action instead. Nothing else here reads an accepted
    /// contribution at all, so the listing was only ever seen on a refused one.
    #[test]
    fn a_contributed_row_is_listed_by_what_it_says() {
        let contributing = format!(
            r#"{MANIFEST}
[requires]
capabilities = ["doctor.contribute"]

[[contribution]]
at        = "doctor.check"
id        = "kavita:claimed"
title     = "Kavita has an administrator"
category  = "credentials"
request   = {{ method = "GET", path = "/api/health" }}
expect    = {{ status = 200 }}
fixture   = "fixtures/guarded.json"
why       = "An unclaimed Kavita hands administrator to whoever asks first."

[[contribution]]
at     = "doctor.remedy"
for    = "kavita:claimed"
id     = "kavita:claim-it"
action = "Open Kavita and create the administrator account"
why    = "Until somebody does, the first caller on the household network becomes it."
"#
        );
        let at = source(
            "contributing",
            &contributing,
            &[
                ("fixtures/guarded.json", guarded()),
                ("fixtures/catalogue.json", catalogue()),
            ],
        );
        let read = claimed(&at).ok();
        let said: Vec<String> = read
            .as_ref()
            .map(|read| {
                read.contributions
                    .iter()
                    .map(|row| format!("{} {} {}", row.at, row.id, row.says))
                    .collect()
            })
            .unwrap_or_default();
        // Built on a line that always runs: a message an assertion computes for itself
        // only runs where it fails, which is a line nothing covers.
        let refused: Vec<String> = read
            .map(|read| read.refusals.iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        assert_eq!(
            said,
            vec![
                "doctor.check kavita:claimed Kavita has an administrator".to_owned(),
                "doctor.remedy kavita:claim-it Open Kavita and create the administrator \
                 account"
                    .to_owned(),
            ],
            "refusals: {refused:?}"
        );
    }

    /// The register a contribution is held against is the doctor's own, not an empty
    /// list handed in from here.
    ///
    /// The rule that a contributed row may not take a bundled identity is only a rule
    /// while the identities it is asked about are the real ones — and nothing else in
    /// this crate would notice if this reader started passing none.
    #[test]
    fn a_contribution_is_held_against_the_identities_the_doctor_actually_holds() {
        let colliding = format!(
            r#"{MANIFEST}
[[contribution]]
at        = "doctor.check"
id        = "{}"
title     = "A row wearing a bundled name"
category  = "storage"
request   = {{ method = "GET", path = "/api/health" }}
expect    = {{ status = 200 }}
fixture   = "fixtures/guarded.json"
why       = "It should be refused for the name rather than for the row."
"#,
            BUNDLED_CHECKS.first().copied().unwrap_or_default()
        );
        let at = source(
            "occupied",
            &colliding,
            &[
                ("fixtures/guarded.json", guarded()),
                ("fixtures/catalogue.json", catalogue()),
            ],
        );
        let said: Vec<String> = claimed(&at)
            .ok()
            .map(|read| read.refusals.iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        assert!(
            said.iter()
                .any(|one| one.contains("bundled row already holds")),
            "got: {said:?}"
        );
    }

    /// Each way a source can be unreadable says which one it was.
    ///
    /// Four refusals with nothing to do with each other: a path holding no plugin, a
    /// file that cannot be read, a manifest this build cannot read, and this build's
    /// own pinned stack failing to publish. Whoever hit one needs a different answer to
    /// each, which is the whole reason they are not one "invalid plugin".
    #[test]
    fn each_way_a_source_is_unreadable_says_which_one_it_was() {
        let said = |problem: Unreadable| problem.to_string();
        assert!(
            said(Unreadable::NoManifest(PathBuf::from("/somewhere"))).contains("/somewhere"),
            "the path it looked in is named"
        );
        assert!(
            said(Unreadable::Unopenable(std::io::Error::other("a disk"))).contains("a disk"),
            "and what the disk said"
        );
        assert!(
            said(Unreadable::Refused(
                lemonfiber_plugin::Error::UnsupportedSchema {
                    found: 9,
                    supported: vec![1],
                }
            ))
            .contains('9'),
            "and the generation a manifest declared"
        );
        assert!(
            said(Unreadable::Stack(super::Ungenerated::Unrenderable)).contains("JSON"),
            "and this build's own failure to publish"
        );
    }

    /// The file and the directory are both what somebody has to hand.
    #[test]
    fn the_manifest_itself_is_a_path_this_reads() {
        let at = whole("named-file");
        assert_eq!(
            claimed(&at.join("plugin.toml")).ok().map(|read| read.id),
            Some("kavita".to_owned())
        );
    }
}
