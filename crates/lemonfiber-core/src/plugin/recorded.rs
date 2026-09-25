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

use serde::Deserialize;
use serde_json::Value;

use crate::within;

/// One answer, as it was recorded.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recording {
    /// The image this came out of, as `image@sha256:…`.
    pub recorded_from: String,
    /// Why this is the answer worth recording — the half a diff cannot show.
    ///
    /// Required rather than optional, and it is the field a reviewer actually reads: a
    /// body of somebody else's JSON says what came back and never why that is the
    /// evidence.
    pub note: String,
    /// What was asked.
    pub request: Asked,
    /// What came back.
    pub response: Answer,
}

/// The request a recording is of.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
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
    #[serde(default)]
    pub accept: Option<String>,
}

/// What came back, in the terms an expectation can constrain.
#[derive(Debug, Clone, Default, Deserialize)]
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
pub fn records(recording: &Recording, asked: &lemonfiber_plugin::Request) -> bool {
    recording.request.method == asked.method
        && recording.request.path == asked.path
        && recording.request.accept == asked.accept
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
