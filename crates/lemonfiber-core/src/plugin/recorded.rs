//! A response somebody recorded, so a claim can be shown where nothing is running.
//!
//! An author's laptop, the catalogue's CI and a reviewer's checkout have no instance
//! of the service a plugin installs, and a claim nobody can check until they own the
//! software is a claim nobody checks. So every probe binds a recording, and the
//! recording is ordinary reviewable data rather than a cassette some tool wrote and
//! only that tool reads.
//!
//! It names the image it came out of, by digest, and that digest is the manifest's own
//! pin. A recording taken from another build is a claim about software nobody is
//! installing, and the drift is silent: it passes, and the service it describes is not
//! the one that will run.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::within;

/// One answer or one run, as it was recorded.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "Written", into = "Written")]
pub struct Recording {
    /// The image this came out of, as `image@sha256:…`.
    pub recorded_from: String,
    /// Why this is the answer worth recording — the half a diff cannot show.
    ///
    /// Required rather than optional, and it is the field a reviewer actually reads: a
    /// body of somebody else's JSON says what came back and never why that is the
    /// evidence.
    pub note: String,
    /// What was asked or run, and what came of it.
    pub of: Of,
}

/// What a recording is of.
#[derive(Debug, Clone)]
pub enum Of {
    /// A request, and the answer to it.
    Request {
        /// What was asked.
        request: Asked,
        /// What came back.
        response: Answer,
    },
    /// A run of the image's own program, and what it came to.
    Run {
        /// What it ran with.
        run: RunAsked,
        /// What it came to.
        result: RunResult,
    },
}

/// A recording as its file holds it, before it is held to being one thing or the other.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    recorded_from: String,
    note: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    request: Option<Asked>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<Answer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    run: Option<RunAsked>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<RunResult>,
}

impl TryFrom<Written> for Recording {
    type Error = String;

    fn try_from(written: Written) -> Result<Self, Self::Error> {
        let of = match (
            written.request,
            written.response,
            written.run,
            written.result,
        ) {
            (Some(request), Some(response), None, None) => Of::Request { request, response },
            (None, None, Some(run), Some(result)) => Of::Run { run, result },
            _ => {
                return Err(
                    "a recording carries request and response, or run and result, and \
                            never both"
                        .to_owned(),
                )
            }
        };
        Ok(Self {
            recorded_from: written.recorded_from,
            note: written.note,
            of,
        })
    }
}

impl From<Recording> for Written {
    fn from(recording: Recording) -> Self {
        let (request, response, run, result) = match recording.of {
            Of::Request { request, response } => (Some(request), Some(response), None, None),
            Of::Run { run, result } => (None, None, Some(run), Some(result)),
        };
        Self {
            recorded_from: recording.recorded_from,
            note: recording.note,
            request,
            response,
            run,
            result,
        }
    }
}

/// The arguments a recorded run was handed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunAsked {
    /// Each argument, as the proof declares it.
    pub args: Vec<String>,
}

/// What a recorded run came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunResult {
    /// The exit status.
    pub exit: i64,
    /// The first 1 KiB of standard output, cut at a character boundary.
    pub stdout_starts_with: String,
}

/// The request a recording is of.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Asked {
    /// The HTTP method.
    pub method: String,
    /// The path on the service.
    pub path: String,
    /// The representation it was asked for, where it asked for one.
    ///
    /// Recorded because a service that negotiates answers two different things at one
    /// path, and a recording that did not say which it asked for would be evidence for
    /// whichever question somebody later pointed at it. Plex answers XML at `/identity`
    /// and JSON at `/identity` — the same second, the same container — and the only
    /// thing that tells the two answers apart is this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accept: Option<String>,
}

/// What came back, in the terms an expectation can constrain.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Answer {
    /// The status.
    pub status: u16,
    /// The headers an expectation may constrain; `content-type` is the one in use.
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// The body as it parsed, or nothing where it did not parse as JSON.
    #[serde(default)]
    pub json: Option<Value>,
    /// The beginning of a body that is not JSON.
    #[serde(default)]
    pub body_starts_with: Option<String>,
}

/// Why a recording could not be run against.
///
/// Every one of these is unproven rather than failed. Nothing about the service has
/// been established either way, and reporting it as a failure would say the service is
/// broken when the recording is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unrunnable(pub String);

/// The recording a path names, beneath the plugin's own source.
///
/// The name comes out of a manifest somebody else wrote, so it is resolved beneath the
/// directory rather than joined to it: a fixture naming a parent would otherwise read a
/// file on the operator's machine that has nothing to do with the plugin.
///
/// # Errors
///
/// [`Unrunnable`] where the name leads outside the source, names nothing, or names
/// something that is not a recording this build can read.
pub fn read(root: &Path, named: &str) -> Result<Recording, Unrunnable> {
    if named.is_empty() {
        return Err(Unrunnable(
            "names no recording, so there is nothing to run it against".to_owned(),
        ));
    }
    let Some(inside) = within::beneath(named) else {
        return Err(Unrunnable(format!(
            "{named} leads outside the plugin's own source"
        )));
    };
    let text = std::fs::read_to_string(root.join(inside))
        .map_err(|unreadable| Unrunnable(format!("{named} could not be read: {unreadable}")))?;
    serde_json::from_str(&text)
        .map_err(|unreadable| Unrunnable(format!("{named} is not a recording: {unreadable}")))
}

/// Whether a recording is of the request an assertion asks.
///
/// A recording of some other call says nothing about this one. The two have drifted,
/// which is a thing to report rather than to judge either way.
///
/// What was asked for is part of *which call this is*, not a detail beside it. A
/// recording taken without `Accept: application/json` from a service that answers XML
/// without it is a recording of the XML, and running a JSON assertion against it would
/// fail a probe over a header nobody sent.
#[must_use]
pub fn records(recorded: &Asked, asked: &lemonfiber_plugin::Request) -> bool {
    recorded.method == asked.method
        && recorded.path == asked.path
        && recorded.accept == asked.accept
}

/// What a recording is of, as a refusal names it.
#[must_use]
pub fn of(recording: &Recording) -> String {
    match &recording.of {
        Of::Request { request, .. } => {
            said(&request.method, &request.path, request.accept.as_deref())
        }
        Of::Run { run, .. } => format!("a run of {}", run.args.join(" ")),
    }
}

/// A run as a refusal and a statement name it.
#[must_use]
pub fn runs(args: &[String]) -> String {
    format!("runs {}", args.join(" "))
}

/// A request as a refusal names it: the verb, the path, and what it asked for.
#[must_use]
pub fn said(method: &str, path: &str, accept: Option<&str>) -> String {
    match accept {
        Some(accept) => format!("{method} {path} as {accept}"),
        None => format!("{method} {path}"),
    }
}

#[cfg(test)]
mod tests;
