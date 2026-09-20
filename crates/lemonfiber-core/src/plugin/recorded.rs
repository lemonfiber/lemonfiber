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
mod tests {
    use std::path::Path;

    use super::{read, records, said, Asked, Recording};

    /// A recording written to a scratch directory, and read back the way a run reads it.
    fn kept(directory: &Path, named: &str, body: &str) {
        let at = directory.join(named);
        if let Some(parent) = at.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(at, body);
    }

    const WHOLE: &str = r#"{
      "recorded_from": "example.invalid/thing@sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
      "note": "An unauthenticated read. A refusal is the pass.",
      "request": { "method": "GET", "path": "/api/v1/series" },
      "response": {
        "status": 401,
        "headers": { "content-type": "application/json" },
        "json": { "error": "Unauthorized" }
      }
    }"#;

    fn scratch(name: &str) -> std::path::PathBuf {
        let at = std::env::temp_dir().join(format!("lemonfiber-recorded-{name}"));
        let _ = std::fs::remove_dir_all(&at);
        let _ = std::fs::create_dir_all(&at);
        at
    }

    #[test]
    fn it_reads_a_recording_as_the_plugins_write_them() {
        let at = scratch("whole");
        kept(&at, "fixtures/guarded.json", WHOLE);
        let held = read(&at, "fixtures/guarded.json");
        let read_back = held.as_ref().map(|one| {
            (
                one.request.clone(),
                one.response.status,
                one.response.json.is_some(),
            )
        });
        assert_eq!(
            read_back,
            Ok((
                Asked {
                    method: "GET".to_owned(),
                    path: "/api/v1/series".to_owned(),
                    accept: None,
                },
                401,
                true
            )),
            "got: {held:?}"
        );
    }

    /// A key this build does not know is refused rather than skipped, because a
    /// recording is part of what a plugin declares — and a misspelled constraint that
    /// is quietly ignored is a proof checking less than it says.
    #[test]
    fn a_field_this_build_does_not_know_is_refused() {
        let at = scratch("unknown");
        kept(
            &at,
            "fixtures/odd.json",
            &WHOLE.replace("\"note\"", "\"notes\""),
        );
        assert!(read(&at, "fixtures/odd.json").is_err());
    }

    #[test]
    fn a_recording_that_is_not_there_says_so_rather_than_failing_a_claim() {
        let at = scratch("absent");
        let said = read(&at, "fixtures/nowhere.json")
            .err()
            .map(|why| why.0)
            .unwrap_or_default();
        assert!(said.contains("fixtures/nowhere.json"), "got: {said}");
        assert!(said.contains("could not be read"), "got: {said}");
    }

    /// A binding that names no recording has nothing to be run against, which is not
    /// the same as a recording that is missing — and saying so costs a sentence.
    #[test]
    fn a_binding_that_names_no_recording_says_that_rather_than_reading_a_directory() {
        let at = scratch("unnamed");
        let said = read(&at, "").err().map(|why| why.0).unwrap_or_default();
        assert!(said.contains("names no recording"), "got: {said}");
    }

    /// A name leading outside the plugin's own source names a file on the operator's
    /// machine, which is not a recording of anything this plugin does.
    #[test]
    fn a_name_reaching_outside_the_plugin_is_refused() {
        let at = scratch("outside");
        let said = read(&at, "../../etc/passwd")
            .err()
            .map(|why| why.0)
            .unwrap_or_default();
        assert!(said.contains("outside the plugin"), "got: {said}");
    }

    /// One request, read the way a manifest's is rather than built here.
    ///
    /// `Request` refuses a field it does not know and takes its own defaults, so a
    /// literal written beside it could name a shape a manifest cannot.
    fn asking(method: &str, path: &str, accept: Option<&str>) -> lemonfiber_plugin::Request {
        let quoted = accept.map_or_else(String::new, |accept| format!(r#", "accept": "{accept}""#));
        let text = format!(r#"{{"method": "{method}", "path": "{path}"{quoted}}}"#);
        let read = serde_json::from_str(&text);
        assert!(read.is_ok(), "the request does not read: {read:?}");
        read.unwrap_or_else(|_| unreachable!("asserted just above"))
    }

    #[test]
    fn a_recording_of_another_call_is_not_of_this_one() {
        let recording: Result<Recording, _> = serde_json::from_str(WHOLE);
        let asked = |method: &str, path: &str| {
            recording
                .as_ref()
                .is_ok_and(|one| records(one, &asking(method, path, None)))
        };
        assert!(asked("GET", "/api/v1/series"), "the call it records");
        assert!(!asked("POST", "/api/v1/series"), "another method");
        assert!(!asked("GET", "/api/v1/libraries"), "another path");
    }

    /// What was asked for is part of which call a recording is of.
    ///
    /// Both directions, because only one of them is the mistake anybody would make: a
    /// recording taken plainly is not evidence for a request that negotiates, and a
    /// recording that negotiated is not evidence for one that did not.
    #[test]
    fn a_recording_that_asked_for_something_else_is_of_another_call() {
        let plain: Result<Recording, _> = serde_json::from_str(WHOLE);
        let negotiated: Result<Recording, _> = serde_json::from_str(&WHOLE.replace(
            r#""path": "/api/v1/series" }"#,
            r#""path": "/api/v1/series", "accept": "application/json" }"#,
        ));
        assert!(
            negotiated.is_ok(),
            "the recording does not read: {negotiated:?}"
        );
        let json = asking("GET", "/api/v1/series", Some("application/json"));
        let bare = asking("GET", "/api/v1/series", None);

        assert!(plain.as_ref().is_ok_and(|one| records(one, &bare)));
        assert!(!plain.as_ref().is_ok_and(|one| records(one, &json)));
        assert!(negotiated.as_ref().is_ok_and(|one| records(one, &json)));
        assert!(!negotiated.as_ref().is_ok_and(|one| records(one, &bare)));
    }

    /// A refusal says what was asked for where anything was, and nothing where not.
    #[test]
    fn a_request_is_named_with_what_it_asked_for() {
        assert_eq!(said("GET", "/identity", None), "GET /identity");
        assert_eq!(
            said("GET", "/identity", Some("application/json")),
            "GET /identity as application/json"
        );
    }
}
