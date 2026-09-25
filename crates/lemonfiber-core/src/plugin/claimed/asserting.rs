//! Running one assertion against the recording it names.
//!
//! The three places an assertion is written — a claim's probe, a `[[proof]]` and a
//! contributed check — ask a service a question and judge what came back, so one
//! runner serves all three. Three runners for one vocabulary would be three things to
//! keep in step, with the one that fell behind quietly deciding less than it said.
//!
//! What the three do *not* share is what a verdict costs. A refuted probe is a service
//! that does not do what it says and a refuted proof is the plugin failing its own
//! condition for being installed, so both stop an install; a refuted check is a check
//! finding the thing it exists to find, on a machine in the state it was recorded in,
//! and stops nothing. That difference is [`super::installs`]'s, not this module's.

use std::path::Path;

use lemonfiber_plugin::{Manifest, Service};

use super::super::judging::judge;
use super::super::recorded::{self, Recording};
use super::Verdict;

/// Which kind of assertion a verdict is about.
///
/// A probe is reported against the capability it demonstrates and is not here. These
/// two are, because neither belongs to a capability: one gates the install of this
/// plugin and the other is a row in a register that runs every day afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Assertion {
    /// What must hold before this plugin is installed.
    Proof,
    /// A check this plugin adds to the doctor's register.
    Check,
}

/// One assertion that is not a probe, and what its recording came to.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Asserted {
    /// Which of the two it is.
    pub kind: Assertion,
    /// What a verdict is reported against.
    pub id: String,
    /// What it establishes, in one line.
    pub says: String,
    /// The service it asks.
    pub service: String,
    /// What the recording said about it.
    pub verdict: Verdict,
}

/// Every proof, against the recording it names.
pub(super) fn proved(
    manifest: &Manifest,
    root: &Path,
    refusals: &mut Vec<lemonfiber_plugin::Violation>,
) -> Vec<Asserted> {
    manifest
        .proofs
        .iter()
        .map(|proof| {
            let service = manifest.asks(proof.service.as_deref());
            Asserted {
                kind: Assertion::Proof,
                id: proof.id.clone(),
                says: proof.title.clone(),
                service: service.map_or_else(String::new, |service| service.id.clone()),
                verdict: recorded_answer(
                    proof.fixture.as_deref(),
                    &proof.request,
                    &proof.expect,
                    service,
                    root,
                    refusals,
                ),
            }
        })
        .collect()
}

/// Every contributed check, against the recording it names.
///
/// A remedy is not one of these. It asks nothing and expects nothing — it is what to do
/// about a check that fired — so running it would be reporting a verdict about a
/// sentence.
pub(super) fn checked(
    manifest: &Manifest,
    root: &Path,
    refusals: &mut Vec<lemonfiber_plugin::Violation>,
) -> Vec<Asserted> {
    manifest
        .contributions
        .iter()
        .filter(|entry| entry.at == lemonfiber_plugin::extension::check())
        .map(|entry| {
            let service = manifest.asks(entry.service.as_deref());
            let (Some(request), Some(expect)) = (&entry.request, &entry.expect) else {
                return Asserted {
                    kind: Assertion::Check,
                    id: entry.id.clone(),
                    says: entry.title.clone().unwrap_or_default(),
                    service: service.map_or_else(String::new, |service| service.id.clone()),
                    verdict: Verdict::Unproven {
                        why: "asks nothing, or says nothing about the answer, so there is \
                              nothing to decide"
                            .to_owned(),
                    },
                };
            };
            let mut answered = |named: Option<&str>| {
                recorded_answer(named, request, expect, service, root, refusals)
            };
            let verdict = match entry.fires_on.as_deref() {
                None => answered(entry.fixture.as_deref()),
                Some(fires_on) if entry.fixture.as_deref().is_none_or(|one| one == fires_on) => {
                    fired(answered(Some(fires_on)), fires_on)
                }
                Some(fires_on) => {
                    let held = answered(entry.fixture.as_deref());
                    both(held, fired(answered(Some(fires_on)), fires_on))
                }
            };
            Asserted {
                kind: Assertion::Check,
                id: entry.id.clone(),
                says: entry.title.clone().unwrap_or_default(),
                service: service.map_or_else(String::new, |service| service.id.clone()),
                verdict,
            }
        })
        .collect()
}

/// What a check came to on the recording it says it fires on.
///
/// Turned over, because firing is what that recording is for: failing there is the
/// check finding what it exists to find, and passing there is a check that finds
/// nothing. A recording that could not be run establishes nothing either way.
fn fired(verdict: Verdict, fires_on: &str) -> Verdict {
    match verdict {
        Verdict::Failed { .. } => Verdict::Passed,
        Verdict::Passed => Verdict::Failed {
            faults: vec![format!(
                "passes on {fires_on}, the recording it says it fires on, so it finds nothing"
            )],
        },
        unproven @ Verdict::Unproven { .. } => unproven,
    }
}

/// Two verdicts about one check, which holds only where both do.
///
/// Unproven wins over failed: a check whose recording could not be run has not been
/// shown to be wrong, and saying it failed would send its author to the wrong file.
fn both(first: Verdict, second: Verdict) -> Verdict {
    match (first, second) {
        (unproven @ Verdict::Unproven { .. }, _) | (_, unproven @ Verdict::Unproven { .. }) => {
            unproven
        }
        (Verdict::Failed { faults: mut all }, Verdict::Failed { faults }) => {
            all.extend(faults);
            Verdict::Failed { faults: all }
        }
        (failed @ Verdict::Failed { .. }, Verdict::Passed)
        | (Verdict::Passed, failed @ Verdict::Failed { .. }) => failed,
        (Verdict::Passed, Verdict::Passed) => Verdict::Passed,
    }
}

/// One assertion, against the recording it names.
///
/// The one runner for the three places an assertion is written, because all three ask a
/// service a question and judge what came back — and three runners for one vocabulary
/// would be three things to keep in step, with the one that fell behind quietly deciding
/// less than it said.
pub(super) fn recorded_answer(
    named: Option<&str>,
    request: &lemonfiber_plugin::Request,
    expect: &lemonfiber_plugin::Expect,
    service: Option<&Service>,
    root: &Path,
    refusals: &mut Vec<lemonfiber_plugin::Violation>,
) -> Verdict {
    let Some(service) = service else {
        return Verdict::Unproven {
            why: "does not settle which of this plugin's services it asks, so there is no image \
                  to hold its recording to"
                .to_owned(),
        };
    };
    let recording = match recorded::read(root, named.unwrap_or_default()) {
        Ok(recording) => recording,
        Err(unrunnable) => return Verdict::Unproven { why: unrunnable.0 },
    };
    let named = named.unwrap_or_default();
    if let Some(wrong) = elsewhere(&recording, service, named) {
        // Refused *rather than run against*. Judging it anyway would report an assertion
        // as answered by an image nobody is installing, which is the passing verdict this
        // refusal exists to stop somebody reading.
        // The whole refusal rather than its message, so the verdict names the recording
        // it is about: one reported unproven beside a dozen others is read on its own
        // line, away from the refusal that explains it.
        let why = wrong.to_string();
        refusals.push(wrong);
        return Verdict::Unproven { why };
    }
    if !recorded::records(&recording, request) {
        return Verdict::Unproven {
            why: format!(
                "{named} records {}, and this asks {}",
                recorded::said(
                    &recording.request.method,
                    &recording.request.path,
                    recording.request.accept.as_deref()
                ),
                recorded::said(&request.method, &request.path, request.accept.as_deref())
            ),
        };
    }
    let faults = judge(expect, &recording.response);
    if faults.is_empty() {
        Verdict::Passed
    } else {
        Verdict::Failed { faults }
    }
}

/// A recording taken from an image this manifest does not install.
///
/// Refused rather than run against. It passes, and the service it describes is not the
/// one that will run — which is the shape a pin moved without re-recording takes, and
/// nothing else would notice it.
fn elsewhere(
    recording: &Recording,
    service: &Service,
    named: &str,
) -> Option<lemonfiber_plugin::Violation> {
    let pinned = format!("{}@{}", service.image, service.digest);
    if recording.recorded_from == pinned {
        return None;
    }
    Some(lemonfiber_plugin::Violation {
        location: named.to_owned(),
        message: format!(
            "was recorded from {}, and this manifest installs {pinned}; a recording of another \
             build passes while describing software nobody is installing",
            recording.recorded_from
        ),
    })
}

#[cfg(test)]
mod tests;
