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

use lemonfiber_plugin::{Declaration, Manifest, Service};

use super::super::judging::{faults, Fault};
use super::super::recorded::{self, Recording};
use super::{FailingAsDeclared, Verdict};

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

/// Every proof, against the recordings it names.
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
            let asserting = Asserting {
                at: format!("proof {}", proof.id),
                fixture: proof.fixture.as_deref(),
                request: &proof.request,
                expect: &proof.expect,
                expected: &proof.expected,
            };
            Asserted {
                kind: Assertion::Proof,
                id: proof.id.clone(),
                says: proof.title.clone(),
                service: service.map_or_else(String::new, |service| service.id.clone()),
                verdict: assessed(&asserting, service, root, refusals),
            }
        })
        .collect()
}

/// Every contributed check, against the recordings it names.
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
            let verdict = match (&entry.request, &entry.expect) {
                (Some(request), Some(expect)) => {
                    let asserting = Asserting {
                        at: format!("contribution {}", entry.id),
                        fixture: entry.fixture.as_deref(),
                        request,
                        expect,
                        expected: &entry.expected,
                    };
                    assessed(&asserting, service, root, refusals)
                }
                _ => Verdict::Unproven {
                    why: "asks nothing, or says nothing about the answer, so there is \
                          nothing to decide"
                        .to_owned(),
                },
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

/// One assertion that is not a probe, as it is run: where it was declared, what it
/// asks, and the recordings it names.
struct Asserting<'a> {
    /// Where the manifest declares it, as a refusal places it.
    at: String,
    /// The recording it holds itself to.
    fixture: Option<&'a str>,
    /// What it asks.
    request: &'a lemonfiber_plugin::Request,
    /// What the answer must be.
    expect: &'a lemonfiber_plugin::Expect,
    /// The recordings it is declared to fail on.
    expected: &'a [Declaration],
}

/// What an assertion came to across every recording it names.
///
/// Its own recording is held to the expectation as written, unless a declaration names
/// that same recording: a declaration is about the recording it names and changes the
/// verdict there and nowhere else. Every declared recording is judged on every
/// constraint, so a failure that is not the declared one is seen.
fn assessed(
    one: &Asserting,
    service: Option<&Service>,
    root: &Path,
    refusals: &mut Vec<lemonfiber_plugin::Violation>,
) -> Verdict {
    let mut verdicts = Vec::new();
    let own = one
        .fixture
        .filter(|own| !one.expected.iter().any(|declared| declared.fixture == *own));
    if let Some(own) = own {
        verdicts.push(recorded_answer(
            Some(own),
            one.request,
            one.expect,
            service,
            root,
            refusals,
        ));
    }
    for declaration in one.expected {
        verdicts.push(declared(one, declaration, service, root, refusals));
    }
    match verdicts.into_iter().reduce(both) {
        Some(verdict) => verdict,
        // Names no recording at all, which the runner says in its own words.
        None => recorded_answer(None, one.request, one.expect, service, root, refusals),
    }
}

/// What an assertion came to on a recording it is declared to fail on.
///
/// A recording the plugin does not carry is refused as well as unproven: the manifest
/// declares a failure on something nobody can read, and the run could not look.
fn declared(
    one: &Asserting,
    declaration: &Declaration,
    service: Option<&Service>,
    root: &Path,
    refusals: &mut Vec<lemonfiber_plugin::Violation>,
) -> Verdict {
    let named = declaration.fixture.as_str();
    let carried = crate::within::beneath(named).is_some_and(|inside| root.join(inside).is_file());
    if !named.trim().is_empty() && !carried {
        refusals.push(lemonfiber_plugin::Violation {
            location: format!("{}.expected {named}", one.at),
            message: "names a recording the plugin's source does not hold; a failure is \
                      declared on a recording somebody can read"
                .to_owned(),
        });
    }
    match faulted(
        Some(named),
        one.request,
        one.expect,
        service,
        root,
        refusals,
    ) {
        Err(why) => Verdict::Unproven { why },
        Ok(faults) => as_declared(declaration, faults),
    }
}

/// Whether what failed on a declared recording is the failure the declaration describes.
///
/// It is only where the named constraint, at the named place, is the one thing that
/// failed. Anything else failing there is reported with every fault, whether or not the
/// declared one failed too; the named one holding makes the declaration stale.
fn as_declared(declaration: &Declaration, faults: Vec<Fault>) -> Verdict {
    let fixture = &declaration.fixture;
    let said = match &declaration.place {
        Some(place) => format!("{} at {place}", declaration.constraint.as_str()),
        None => declaration.constraint.as_str().to_owned(),
    };
    let is_named = |fault: &Fault| {
        fault.constraint == declaration.constraint && fault.place == declaration.place
    };
    if !faults.iter().all(is_named) {
        let every: Vec<&str> = faults.iter().map(|fault| fault.said.as_str()).collect();
        return Verdict::Failed {
            faults: vec![format!(
                "{fixture} is declared to fail on {said}, and fails on: {}",
                every.join("; ")
            )],
        };
    }
    match faults.into_iter().next() {
        None => Verdict::Failed {
            faults: vec![format!(
                "{fixture} is declared to fail on {said}, and {said} holds there, so the \
                 declaration is stale; it goes in the change that made the recording pass"
            )],
        },
        Some(fault) => Verdict::FailingAsDeclared {
            declared: vec![FailingAsDeclared {
                fixture: fixture.clone(),
                constraint: declaration.constraint,
                place: declaration.place.clone(),
                held: fault.held,
                reason: declaration.reason.clone(),
            }],
        },
    }
}

/// Two verdicts about one assertion, reported as one.
///
/// Failed wins over everything, because a failure has to fail the run whatever else the
/// other recordings came to; a recording that could not be run is then named among the
/// faults rather than dropped. Unproven wins over failing as declared and over passed,
/// because what was not run was not shown. Failing as declared wins over passed, because
/// it is never counted as a pass.
fn both(first: Verdict, second: Verdict) -> Verdict {
    match (first, second) {
        (Verdict::Failed { faults: mut all }, Verdict::Failed { faults }) => {
            all.extend(faults);
            Verdict::Failed { faults: all }
        }
        (Verdict::Failed { mut faults }, Verdict::Unproven { why })
        | (Verdict::Unproven { why }, Verdict::Failed { mut faults }) => {
            faults.push(why);
            Verdict::Failed { faults }
        }
        (failed @ Verdict::Failed { .. }, _) | (_, failed @ Verdict::Failed { .. }) => failed,
        (Verdict::Unproven { why: first }, Verdict::Unproven { why: second }) => {
            Verdict::Unproven {
                why: format!("{first}; {second}"),
            }
        }
        (unproven @ Verdict::Unproven { .. }, _) | (_, unproven @ Verdict::Unproven { .. }) => {
            unproven
        }
        (
            Verdict::FailingAsDeclared { declared: mut all },
            Verdict::FailingAsDeclared { declared },
        ) => {
            all.extend(declared);
            Verdict::FailingAsDeclared { declared: all }
        }
        (declared @ Verdict::FailingAsDeclared { .. }, Verdict::Passed)
        | (Verdict::Passed, declared @ Verdict::FailingAsDeclared { .. }) => declared,
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
    match faulted(named, request, expect, service, root, refusals) {
        Err(why) => Verdict::Unproven { why },
        Ok(faults) if faults.is_empty() => Verdict::Passed,
        Ok(faults) => Verdict::Failed {
            faults: faults.into_iter().map(|fault| fault.said).collect(),
        },
    }
}

/// Every way the recording an assertion names is not what it declares, or why it could
/// not be run against that recording at all.
fn faulted(
    named: Option<&str>,
    request: &lemonfiber_plugin::Request,
    expect: &lemonfiber_plugin::Expect,
    service: Option<&Service>,
    root: &Path,
    refusals: &mut Vec<lemonfiber_plugin::Violation>,
) -> Result<Vec<Fault>, String> {
    let Some(service) = service else {
        return Err(
            "does not settle which of this plugin's services it asks, so there is no image \
             to hold its recording to"
                .to_owned(),
        );
    };
    let recording =
        recorded::read(root, named.unwrap_or_default()).map_err(|unrunnable| unrunnable.0)?;
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
        return Err(why);
    }
    if !recorded::records(&recording, request) {
        return Err(format!(
            "{named} records {}, and this asks {}",
            recorded::said(
                &recording.request.method,
                &recording.request.path,
                recording.request.accept.as_deref()
            ),
            recorded::said(&request.method, &request.path, request.accept.as_deref())
        ));
    }
    Ok(faults(expect, &recording.response))
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
