//! Each case of each contract a plugin's adapters speak, as its recording answers it.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use async_trait::async_trait;
use lemonfiber_contract::client::Witness;
use lemonfiber_contract::conformance::{filed, Case, Expect, UNKEYED};
use lemonfiber_contract::{Capability, Contracted};
use lemonfiber_ports::http::{Http, Request, Response, Unreachable, JSON};

use crate::recording::{self, Recording};

/// Where a plugin's conformance recordings sit, beneath its source.
pub const RECORDINGS: &str = "conformance";

/// The method every operation is asked with.
pub const ASKED_WITH: &str = "POST";

/// What one case's recording came to.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum Verdict {
    /// The recorded answer is one the contract declares for the case.
    Passed,
    /// It is not, in every way it is not.
    Failed {
        /// Every way the recorded answer is outside the case.
        faults: Vec<String>,
    },
    /// No recording of the case is there.
    Unrecorded,
    /// A recording is there and could not be judged, so it established nothing.
    Unproven {
        /// What stopped it being judged.
        why: String,
    },
}

/// One case, as its recording answered it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
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

impl std::fmt::Display for Verdict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Passed => f.write_str("passed"),
            Self::Failed { faults } => write!(f, "failed: {}", faults.join("; ")),
            Self::Unrecorded => f.write_str("no recording answers it"),
            Self::Unproven { why } => write!(f, "unproven: {why}"),
        }
    }
}

impl std::fmt::Display for Judged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{} — {}", self.contract, self.case, self.verdict)
    }
}

/// One adapter of a plugin, as much of it as judging asks: what it speaks, and what the
/// upstream it answers for is pinned at and files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Speaker {
    /// The adapter's service.
    pub service: String,
    /// Each contract it speaks, as `capability@major`.
    pub speaks: Vec<String>,
    /// The image of the upstream it answers for, as `image@digest`, where it has one.
    pub pinned: Option<String>,
    /// The media types that upstream files.
    pub files: Vec<String>,
}

/// One recording a plugin owes: a case of a contract one of its adapters speaks.
#[derive(Debug, Clone)]
pub struct Owed<'c> {
    /// The adapter's service.
    pub service: String,
    /// The contract it speaks, as `capability@major`.
    pub contract: String,
    /// The case.
    pub case: Case<'c>,
    /// Where its recording is, beneath the plugin's source.
    pub recording: String,
    /// The image of the upstream the adapter answers for, as a recording names it.
    pub pinned: Option<String>,
}

/// Every recording `speakers` owe: each case of each contract `published` that one of them
/// speaks, in the order they and the contracts list them. A contract a speaker speaks that
/// `published` does not hold is the manifest's to refuse, and owes nothing here.
#[must_use]
pub fn owed<'c>(speakers: &[Speaker], published: &'c [Capability]) -> Vec<Owed<'c>> {
    let mut owed = Vec::new();
    for speaker in speakers {
        for named in &speaker.speaks {
            let Some(spoken) = published
                .iter()
                .find(|one| lemonfiber_contract::spoken(one.name, one.major) == *named)
            else {
                continue;
            };
            owed.extend(filed(spoken, &speaker.files).into_iter().map(|case| Owed {
                service: speaker.service.clone(),
                contract: named.clone(),
                recording: recording_of(named, &case.case),
                case,
                pinned: speaker.pinned.clone(),
            }));
        }
    }
    owed
}

/// Every recording `speakers` owe, as the recording beneath `root` answers it.
pub async fn judged(root: &Path, speakers: &[Speaker], published: &[Capability]) -> Vec<Judged> {
    let owed = owed(speakers, published);
    let mut judged = Vec::new();
    for one in &owed {
        let verdict = if root.join(&one.recording).exists() {
            match recording::read(root, &one.recording) {
                Ok(recorded) => verdict_of(one, &recorded).await,
                Err(unrunnable) => unproven(&unrunnable.0),
            }
        } else if one.case.since > recorded_minor(root, &owed, one) {
            Verdict::Passed
        } else {
            Verdict::Unrecorded
        };
        judged.push(Judged {
            contract: one.contract.clone(),
            case: one.case.case.clone(),
            recording: one.recording.clone(),
            verdict,
        });
    }
    judged
}

/// [`judged`] driven to its end where nothing else awaits: a build script, which has no
/// runtime of its own. The client bounds every answer in time, so it runs on a runtime
/// of one thread with a clock.
///
/// # Errors
///
/// Where that runtime cannot be made.
pub fn judged_now(
    root: &Path,
    speakers: &[Speaker],
    published: &[Capability],
) -> std::io::Result<Vec<Judged>> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()?;
    Ok(runtime.block_on(judged(root, speakers, published)))
}

/// Where the recording of `case` of `contract` sits, beneath the plugin's source.
#[must_use]
pub fn recording_of(contract: &str, case: &str) -> String {
    format!("{RECORDINGS}/{contract}/{case}.json")
}

/// The highest minor among the operations of `one`'s contract its adapter holds a
/// recording for: the minor of the core it was recorded against. An operation added after
/// it is not a case of that minor.
fn recorded_minor(root: &Path, owed: &[Owed<'_>], one: &Owed<'_>) -> u32 {
    owed.iter()
        .filter(|other| other.service == one.service && other.contract == one.contract)
        .filter(|other| root.join(&other.recording).exists())
        .map(|other| other.case.since)
        .max()
        .unwrap_or_default()
}

/// What `recording` comes to for the case `one` owes.
async fn verdict_of(one: &Owed<'_>, recording: &Recording) -> Verdict {
    let case = &one.case;
    let Some(pinned) = one.pinned.as_deref() else {
        return unproven("the adapter names no upstream it was recorded against");
    };
    if recording.recorded_from != pinned {
        return unproven(&format!(
            "was recorded from {}, and the upstream it fronts is pinned at {pinned}",
            recording.recorded_from
        ));
    }
    let recording::Of::Request {
        request: asked,
        response,
    } = &recording.of
    else {
        return unproven(&format!(
            "records a run, {}, and a contract's case is asked over a request",
            recording::of(recording)
        ));
    };
    let path = case.operation.path();
    if asked.method != ASKED_WITH || asked.path != path || asked.accept.as_deref() != Some(JSON) {
        return unproven(&format!(
            "records {}, and the case asks {}",
            recording::said(&asked.method, &asked.path, asked.accept.as_deref()),
            recording::said(ASKED_WITH, &path, Some(JSON))
        ));
    }
    match case.expect {
        Expect::Status(statuses) if statuses.contains(&response.status) => Verdict::Passed,
        Expect::Status(statuses) => Verdict::Failed {
            faults: vec![format!(
                "answered {}, and a call without the key is refused with {statuses:?}",
                response.status
            )],
        },
        Expect::Conforms if UNKEYED.contains(&response.status) => Verdict::Failed {
            faults: vec!["refused the plugin's own key".to_owned()],
        },
        Expect::Conforms => {
            let told = Arc::new(Told::default());
            let client = Contracted::new(
                Arc::new(Replayed(answered(response))),
                "http://recorded",
                &one.service,
                "recorded",
            )
            .witnessed_by(told.clone());
            let _ = case.operation.judged(&client).await;
            verdict(told.said())
        }
    }
}

/// A case's verdict from what was told of reading its answer.
///
/// An answer the contract declares passes, a refusal it declares included, as an upstream
/// refusing the credential the adapter holds for it. One outside the contract fails with
/// what was outside it.
fn verdict(told: Vec<String>) -> Verdict {
    if told.is_empty() {
        Verdict::Passed
    } else {
        Verdict::Failed { faults: told }
    }
}

/// Unproven, for `why`.
fn unproven(why: &str) -> Verdict {
    Verdict::Unproven {
        why: why.to_owned(),
    }
}

/// The recorded answer, as a transport hands it back.
fn answered(answer: &recording::Answer) -> Response {
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
