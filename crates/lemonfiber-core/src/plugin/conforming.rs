//! Whether a plugin's adapters answer the contracts they speak, judged from the
//! recordings the plugin ships and nothing else.
//!
//! Each case of each contract an adapter speaks has one recording beside the manifest,
//! at `conformance/<capability>@<major>/<case>.json`, taken against the upstream the
//! adapter `fronts` at the digest the manifest pins it by. A recorded answer is read by
//! the very client the core asks a running adapter with, so what passes here is what
//! the core would read and what fails is what it would refuse.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use lemonfiber_contract::client::Witness;
use lemonfiber_contract::conformance::{cases, Case, Expect};
use lemonfiber_contract::{Contracted, Failure};
use lemonfiber_plugin::{Manifest, Service, Violation};

use super::claimed::{sourced, Unreadable, Verdict};
use super::recorded::{self, Recording};
use crate::doctor::BUNDLED_CHECKS;
use crate::ports::http::{Http, Request, Response, Unreachable, JSON};

/// Where a plugin's conformance recordings sit, beneath its source.
const RECORDINGS: &str = "conformance";

/// The method every operation is asked with.
const ASKED_WITH: &str = "POST";

/// What a plugin's recordings say about the contracts its adapters speak.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Conformed {
    /// The plugin's id.
    pub id: String,
    /// Everything the manifest is refused for, which no recording can make up for.
    pub refusals: Vec<Violation>,
    /// Every case of every contract an adapter speaks, as its recording answered it.
    pub cases: Vec<Judged>,
    /// Whether nothing is refused and every case passed.
    pub conforms: bool,
}

/// One case, as its recording answered it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Judged {
    /// The contract it is a case of, as `capability@major`.
    pub contract: String,
    /// The case.
    pub case: String,
    /// Where its recording is, beneath the plugin's source.
    pub recording: String,
    /// What it came to.
    pub verdict: Verdict,
}

/// What the plugin whose source is at this path conforms to, judged from its recordings.
///
/// # Errors
///
/// [`Unreadable`] where there is no manifest at the path, or this build cannot read it.
pub async fn conformed(path: &Path) -> Result<Conformed, Unreadable> {
    let (root, manifest) = sourced(path)?;
    let refusals = lemonfiber_plugin::refusals(&manifest, BUNDLED_CHECKS);
    let published = lemonfiber_contract::capabilities::all();
    let mut judged = Vec::new();
    for service in manifest
        .services
        .iter()
        .filter(|one| !one.speaks.is_empty())
    {
        let pinned = upstream(&manifest, service);
        for named in &service.speaks {
            let Some(spoken) = published
                .iter()
                .find(|one| lemonfiber_contract::spoken(one.name, one.major) == *named)
            else {
                continue;
            };
            for case in cases(spoken) {
                let recording = format!("{RECORDINGS}/{named}/{}.json", case.case);
                let verdict = match recorded::read(&root, &recording) {
                    Err(unrunnable) => Verdict::Unproven { why: unrunnable.0 },
                    Ok(recorded) => {
                        let asked = Asked {
                            service,
                            case: &case,
                            pinned: pinned.as_deref(),
                        };
                        asked.judged(&recorded).await
                    }
                };
                judged.push(Judged {
                    contract: named.clone(),
                    case: case.case.clone(),
                    recording,
                    verdict,
                });
            }
        }
    }
    let conforms = refusals.is_empty() && judged.iter().all(|one| one.verdict == Verdict::Passed);
    Ok(Conformed {
        id: manifest.plugin.id.clone(),
        refusals,
        cases: judged,
        conforms,
    })
}

/// The image the service an adapter fronts is pinned by, as a recording names it.
fn upstream(manifest: &Manifest, adapter: &Service) -> Option<String> {
    let fronted = adapter.fronts.as_deref()?;
    manifest
        .services
        .iter()
        .find(|one| one.id == fronted)
        .map(|one| format!("{}@{}", one.image, one.digest))
}

/// One case of one contract an adapter speaks, as its recording is judged against.
struct Asked<'a> {
    service: &'a Service,
    case: &'a Case<'a>,
    pinned: Option<&'a str>,
}

impl Asked<'_> {
    /// What the recording comes to for this case.
    async fn judged(&self, recording: &Recording) -> Verdict {
        let Some(pinned) = self.pinned else {
            return unproven("the adapter names no upstream it was recorded against");
        };
        if recording.recorded_from != pinned {
            return unproven(&format!(
                "was recorded from {}, and the upstream it fronts is pinned at {pinned}",
                recording.recorded_from
            ));
        }
        let operation = self.case.operation;
        let asked = &recording.request;
        let path = operation.path();
        if asked.method != ASKED_WITH || asked.path != path || asked.accept.as_deref() != Some(JSON)
        {
            return unproven(&format!(
                "records {}, and the case asks {}",
                recorded::said(&asked.method, &asked.path, asked.accept.as_deref()),
                recorded::said(ASKED_WITH, &path, Some(JSON))
            ));
        }
        match self.case.expect {
            Expect::Status(statuses) if statuses.contains(&recording.response.status) => {
                Verdict::Passed
            }
            Expect::Status(statuses) => Verdict::Failed {
                faults: vec![format!(
                    "answered {}, and a call without the key is refused with {statuses:?}",
                    recording.response.status
                )],
            },
            Expect::Conforms => {
                let told = Arc::new(Told::default());
                let client = Contracted::new(
                    Arc::new(Replayed(answered(recording))),
                    "http://recorded",
                    &self.service.id,
                    "recorded",
                )
                .witnessed_by(told.clone());
                let read = operation.judged(&client).await;
                verdict(&read, told.said())
            }
        }
    }
}

/// A case's verdict from what reading its answer came to and what was told of it.
///
/// An answer the contract declares passes, a refusal it declares included. One outside
/// the contract fails with what was outside it, and so does an adapter refusing the
/// plugin's own key, which is no answer to the operation at all.
fn verdict(read: &Result<(), Failure>, told: Vec<String>) -> Verdict {
    match read {
        Err(Failure::Unauthorised { .. }) => Verdict::Failed {
            faults: vec!["refused the plugin's own key".to_owned()],
        },
        _ if !told.is_empty() => Verdict::Failed { faults: told },
        _ => Verdict::Passed,
    }
}

/// Unproven, for `why`.
fn unproven(why: &str) -> Verdict {
    Verdict::Unproven {
        why: why.to_owned(),
    }
}

/// The recorded answer, as a transport hands it back.
fn answered(recording: &Recording) -> Response {
    let answer = &recording.response;
    Response {
        status: answer.status,
        headers: answer
            .headers
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
        body: answer
            .json
            .as_ref()
            .map(ToString::to_string)
            .or_else(|| answer.body_starts_with.clone())
            .unwrap_or_default(),
    }
}

/// A transport answering every request with one recorded answer.
struct Replayed(Response);

#[async_trait]
impl Http for Replayed {
    async fn send(&self, _request: &Request) -> Result<Response, Unreachable> {
        Ok(self.0.clone())
    }
}

/// Everything the client said was outside the contract.
#[derive(Default)]
struct Told(Mutex<Vec<String>>);

impl Told {
    fn said(&self) -> Vec<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Witness for Told {
    fn nonconforming(&self, _operation: &str, why: &str) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(why.to_owned());
    }
}

#[cfg(test)]
mod tests;
