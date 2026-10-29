use std::path::{Path, PathBuf};

use lemonfiber_manifest::{Claimed, Manifest};

use super::{judged, said};
use crate::plugin::Verdict;

const STACK: &str = lemonfiber_bundled::STACK;

/// A service of a part the bundle assembles, which the stack runs as its own.
const ASSEMBLED: &str = "flaresolverr";

/// A service of a part the bundle installs, kept in the stack until it moves.
const KEPT: &str = "jellyfin";

/// Both probes `media.serve` declares, bound under the assembled service's own
/// recordings.
const CLAIM: &str = r#"
capability = "media.serve"

[[probe]]
id = "guarded"
request = { method = "GET", path = "/Items" }
expect = { status = 401 }
fixture = "recordings/flaresolverr/guarded.json"

[[probe]]
id = "catalogue"
request = { method = "GET", path = "/Items" }
expect = { status = 200, json_has_keys = ["Items"] }
fixture = "recordings/flaresolverr/catalogue.json"
"#;

/// The shipped stack, with [`ASSEMBLED`] alone providing and claiming `media.serve` as
/// `claim` writes it.
fn claiming(claim: &str) -> Manifest {
    let mut manifest = Manifest::from_toml(STACK).unwrap_or_else(|why| unreachable!("{why}"));
    for service in &mut manifest.services {
        if service.id == ASSEMBLED {
            service.provides = vec!["media.serve".to_owned()];
            service.claim = vec![Claimed(
                toml::from_str(claim).unwrap_or_else(|why| unreachable!("{why}")),
            )];
        } else {
            service.provides = Vec::new();
            service.claim = Vec::new();
        }
    }
    manifest
}

/// Where [`ASSEMBLED`]'s image is pinned in the shipped stack.
fn pinned(manifest: &Manifest) -> String {
    manifest
        .services
        .iter()
        .find(|service| service.id == ASSEMBLED)
        .map(lemonfiber_manifest::Service::reference)
        .unwrap_or_default()
}

/// A stack directory holding the recordings named, each answering `status` with `body`.
fn recorded(tag: &str, from: &str, answers: &[(&str, u16, &str)]) -> PathBuf {
    let root = lemonfiber_fixtures::scratch::Scratch::named(tag).kept();
    let under = root.join("recordings/flaresolverr");
    let _ = std::fs::create_dir_all(&under);
    for (name, status, body) in answers {
        let recording = format!(
            r#"{{"recorded_from":"{from}","note":"why this answer","request":{{"method":"GET","path":"/Items"}},"response":{{"status":{status},"json":{body}}}}}"#
        );
        let _ = std::fs::write(under.join(name), recording);
    }
    root
}

/// Remove a scratch stack directory.
fn gone(root: &Path) {
    let _ = std::fs::remove_dir_all(root);
}

/// Recordings that answer as the claim says demonstrate it.
#[test]
fn recordings_that_answer_as_claimed_demonstrate_it() {
    let manifest = claiming(CLAIM);
    let root = recorded(
        "bundled-passes",
        &pinned(&manifest),
        &[
            ("guarded.json", 401, "null"),
            ("catalogue.json", 200, r#"{"Items":[]}"#),
        ],
    );
    let report = judged(&manifest, &root);
    gone(&root);

    assert!(report.holds(), "{report:?}");
    assert_eq!(report.judged.len(), 2);
    assert!(report
        .judged
        .iter()
        .all(|one| one.verdict == Verdict::Passed));
}

/// A recording that contradicts its probe refutes the claim, naming how.
#[test]
fn a_recording_that_contradicts_its_probe_refutes_the_claim() {
    let manifest = claiming(CLAIM);
    let root = recorded(
        "bundled-refuted",
        &pinned(&manifest),
        &[
            ("guarded.json", 200, r#"{"Items":[]}"#),
            ("catalogue.json", 200, r#"{"Items":[]}"#),
        ],
    );
    let report = judged(&manifest, &root);
    gone(&root);

    assert!(!report.holds());
    let refuted: Vec<&str> = report.refuted().map(|one| one.probe.as_str()).collect();
    assert_eq!(refuted, vec!["guarded"]);
}

/// A probe with no recording is unproven, and unproven refuses nothing.
#[test]
fn a_probe_with_no_recording_is_unproven_and_refuses_nothing() {
    let manifest = claiming(CLAIM);
    let root = recorded(
        "bundled-unproven",
        &pinned(&manifest),
        &[("guarded.json", 401, "null")],
    );
    let report = judged(&manifest, &root);
    gone(&root);

    assert!(report.holds(), "{report:?}");
    let unproven: Vec<&str> = report.unproven().map(|one| one.probe.as_str()).collect();
    assert_eq!(unproven, vec!["catalogue"]);
}

/// A recording taken from an image the stack does not pin is refused, not trusted.
#[test]
fn a_recording_of_another_image_is_refused() {
    let manifest = claiming(CLAIM);
    let root = recorded(
        "bundled-elsewhere",
        "example.invalid/elsewhere@sha256:0000000000000000000000000000000000000000000000000000000000000000",
        &[
            ("guarded.json", 401, "null"),
            ("catalogue.json", 200, r#"{"Items":[]}"#),
        ],
    );
    let report = judged(&manifest, &root);
    gone(&root);

    assert!(!report.holds());
    assert_eq!(report.refused.len(), 2, "{:?}", report.refused);
    assert_eq!(report.unproven().count(), 2);
}

/// A claim the contract will not bind is refused before anything is judged.
#[test]
fn a_claim_the_contract_will_not_bind_is_refused() {
    let one_probe = CLAIM
        .split("[[probe]]")
        .take(2)
        .collect::<Vec<_>>()
        .join("[[probe]]");
    let manifest = claiming(&one_probe);
    let root = recorded("bundled-unbound", &pinned(&manifest), &[]);
    let report = judged(&manifest, &root);
    gone(&root);

    assert!(!report.holds());
    assert!(report
        .refused
        .iter()
        .any(|refused| refused.message.contains("binds no probe catalogue")));
}

/// A capability an assembled service provides with no claim is named as unclaimed and
/// refuses the stack, even with nothing else wrong: a declaration nothing demonstrates is
/// one nothing holds.
#[test]
fn an_assembled_service_providing_with_no_claim_refuses_the_stack() {
    let mut manifest = claiming(CLAIM);
    for service in &mut manifest.services {
        service.claim = Vec::new();
    }
    let report = judged(&manifest, Path::new("/lemonfiber/no/such/stack"));

    assert!(!report.holds(), "{report:?}");
    assert!(
        report.refused.is_empty() && report.judged.is_empty(),
        "{report:?}"
    );
    assert_eq!(
        report.unclaimed,
        vec![super::Unclaimed {
            service: ASSEMBLED.to_owned(),
            capability: "media.serve".to_owned(),
        }]
    );
    assert_eq!(
        said(&report),
        vec![
            "refused: flaresolverr provides media.serve, and no claim demonstrates it".to_owned(),
            "1 refused, 0 refuted, 0 unproven, 0 demonstrated".to_owned(),
        ]
    );
}

/// Each answer is said in its own words, worst first, with a count of each.
#[test]
fn the_report_says_each_answer_worst_first_with_a_count() {
    let manifest = claiming(CLAIM);
    let root = recorded(
        "bundled-said",
        &pinned(&manifest),
        &[("guarded.json", 200, r#"{"Items":[]}"#)],
    );
    let mut report = judged(&manifest, &root);
    gone(&root);
    report.unclaimed.push(super::Unclaimed {
        service: "sonarr".to_owned(),
        capability: "library.manage".to_owned(),
    });
    report.refused.push(lemonfiber_plugin::Violation {
        location: "service sonarr".to_owned(),
        message: "is wrong".to_owned(),
    });
    let lines = said(&report);

    assert!(
        lines
            .first()
            .is_some_and(|line| line.starts_with("refused: ")),
        "{lines:?}"
    );
    assert!(
        lines.get(2).is_some_and(
            |line| line.starts_with("refuted: flaresolverr media.serve probe guarded")
        ),
        "{lines:?}"
    );
    assert!(
        lines
            .get(1)
            .is_some_and(|line| line
                == "refused: sonarr provides library.manage, and no claim demonstrates it"),
        "{lines:?}"
    );
    assert!(lines
        .iter()
        .any(|line| line.starts_with("unproven: flaresolverr media.serve probe catalogue")));
    assert_eq!(
        lines.last().map(String::as_str),
        Some("2 refused, 1 refuted, 1 unproven, 0 demonstrated")
    );
}

/// A probe the recording answers is said as demonstrated.
#[test]
fn a_probe_the_recording_answers_is_said_as_demonstrated() {
    let manifest = claiming(CLAIM);
    let root = recorded(
        "bundled-said-shown",
        &pinned(&manifest),
        &[
            ("guarded.json", 401, "null"),
            ("catalogue.json", 200, r#"{"Items":[]}"#),
        ],
    );
    let lines = said(&judged(&manifest, &root));
    gone(&root);

    assert!(lines
        .iter()
        .any(|line| line == "demonstrated: flaresolverr media.serve probe guarded"));
    assert_eq!(
        lines.last().map(String::as_str),
        Some("0 refused, 0 refuted, 0 unproven, 2 demonstrated")
    );
}

/// A probe bound to a run is judged against the run its recording took, and one that
/// declares neither a request nor a run is unproven.
#[test]
fn a_probe_bound_to_a_run_is_judged_against_the_run_recorded() {
    let manifest = claiming(
        &CLAIM
            .replace(
                r#"id = "guarded"
request = { method = "GET", path = "/Items" }
expect = { status = 401 }"#,
                r#"id = "guarded"
run = { args = ["--version"] }
expect = { exit = 0, stdout_starts_with = "10.11" }"#,
            )
            .replace(
                r#"request = { method = "GET", path = "/Items" }
expect = { status = 200"#,
                "expect = { status = 200",
            ),
    );
    let root = recorded("bundled-runs", &pinned(&manifest), &[]);
    let _ = std::fs::write(
        root.join("recordings/flaresolverr/guarded.json"),
        format!(
            r#"{{"recorded_from":"{}","note":"its release","run":{{"args":["--version"]}},"result":{{"exit":0,"stdout_starts_with":"10.11.11\n"}}}}"#,
            pinned(&manifest)
        ),
    );
    let report = judged(&manifest, &root);
    gone(&root);

    let verdicts: Vec<(&str, &Verdict)> = report
        .judged
        .iter()
        .map(|one| (one.probe.as_str(), &one.verdict))
        .collect();
    assert_eq!(
        verdicts,
        vec![
            ("guarded", &Verdict::Passed),
            (
                "catalogue",
                &Verdict::Unproven {
                    why: crate::plugin::claimed::ASKS_NO_ONE_THING.to_owned()
                }
            ),
        ]
    );
}

/// A capability a kept service provides is not judged here: the part's adapter carries
/// its claims and is judged against its own repository's recordings and live check.
#[test]
fn a_kept_service_providing_with_no_claim_is_not_judged_here() {
    let mut manifest = claiming(CLAIM);
    for service in &mut manifest.services {
        service.claim = Vec::new();
        service.provides = if service.id == KEPT {
            vec!["media.serve".to_owned()]
        } else {
            Vec::new()
        };
    }
    let report = judged(&manifest, Path::new("/lemonfiber/no/such/stack"));

    assert!(report.holds(), "{report:?}");
    assert!(report.unclaimed.is_empty(), "{report:?}");
}
