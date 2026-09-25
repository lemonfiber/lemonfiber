use std::time::Duration;

use super::fixtures::{finding, problem};
use super::{Category, Overall, Verdict};
use crate::error::Remedy;

/// The two promises about time meet at this number, and a change to either has to be
/// made knowing about the other. A single check may ask for longer than the default,
/// and not for so long that one slow disk breaks the thirty seconds a full
/// non-disruptive run is meant to finish in — which holds only because the checks run
/// concurrently, so a run costs its slowest check rather than their sum.
#[test]
fn a_filesystem_may_wait_longer_than_a_container_command_but_not_past_a_whole_run() {
    assert!(
        super::FILESYSTEM_BUDGET > super::CHECK_BUDGET,
        "a disk that has spun down needs longer than a container command"
    );
    assert!(
        super::FILESYSTEM_BUDGET <= Duration::from_secs(30),
        "a full non-disruptive run is meant to finish inside thirty seconds"
    );
}

#[test]
fn every_category_survives_a_round_trip_through_its_name() {
    for category in [
        Category::Environment,
        Category::Storage,
        Category::Network,
        Category::Vpn,
        Category::Credentials,
        Category::Services,
        Category::Providers,
        Category::Queue,
        Category::Config,
    ] {
        assert_eq!(Category::parse(category.as_str()), Some(category));
    }
}

#[test]
fn a_name_lemonfiber_does_not_know_is_not_a_category() {
    assert_eq!(Category::parse("nonsense"), None);
}

/// The machine-readable output is a versioned contract, so the exact words
/// on the wire are pinned here rather than left to chance.
#[test]
fn every_outcome_has_a_name_on_the_wire() {
    use crate::model::DoctorReport;
    let report = DoctorReport {
        overall: Overall::Broken,
        findings: vec![
            finding(
                "egress",
                Verdict::Pass {
                    note: Some("185.65.1.1".to_owned()),
                },
            ),
            finding("port", Verdict::Warn(problem())),
            finding("leak", Verdict::Fail(problem())),
            finding(
                "killswitch",
                Verdict::Unverified {
                    reason: "untested".to_owned(),
                    remedy: Remedy::new("run it"),
                },
            ),
            finding(
                "forwarded-port",
                Verdict::Skipped {
                    reason: "no port forwarding".to_owned(),
                },
            ),
        ],
    };
    let json = serde_json::to_string(&report).unwrap_or_default();
    assert!(json.contains(r#""overall":"broken""#));
    assert!(json.contains(r#""category":"vpn""#));
    for outcome in ["pass", "warn", "fail", "unverified", "skipped"] {
        assert!(
            json.contains(&format!(r#""outcome":"{outcome}""#)),
            "{outcome} should name itself in {json}"
        );
    }
}

/// The fields the schema declares for the verdict that names this outcome.
fn described(schema: &serde_json::Value, outcome: &str) -> std::collections::BTreeSet<String> {
    let branches = schema
        .get("oneOf")
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    branches
        .iter()
        .filter(|branch| {
            branch
                .pointer("/properties/outcome/const")
                .and_then(serde_json::Value::as_str)
                == Some(outcome)
        })
        .flat_map(|branch| {
            branch
                .pointer("/properties")
                .and_then(serde_json::Value::as_object)
                .map(|fields| fields.keys().cloned().collect::<Vec<String>>())
                .unwrap_or_default()
        })
        .collect()
}

/// Every field a verdict writes is a field the schema describes.
///
/// Naming the outcome is not enough. `Warn` and `Fail` write a whole diagnosis
/// beside the tag, and a schema that mentions only the tag reads as a verdict
/// that carries nothing — which is what a generator then emits, leaving the
/// summary, the meaning and the remedies out of the only two verdicts that
/// have them.
#[test]
fn every_verdict_describes_the_fields_it_writes() {
    let schema = serde_json::to_value(schemars::schema_for!(Verdict)).unwrap_or_default();
    for verdict in [
        Verdict::Pass {
            note: Some("185.65.1.1".to_owned()),
        },
        Verdict::Warn(problem()),
        Verdict::Fail(problem()),
        Verdict::Unverified {
            reason: "untested".to_owned(),
            remedy: Remedy::new("run it"),
        },
        Verdict::Skipped {
            reason: "no port forwarding".to_owned(),
        },
    ] {
        let document = serde_json::to_value(&verdict).unwrap_or_default();
        let written: std::collections::BTreeSet<String> = document
            .as_object()
            .map(|fields| fields.keys().cloned().collect())
            .unwrap_or_default();
        let outcome = document
            .pointer("/outcome")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let described = described(&schema, outcome);

        assert!(
            written.is_subset(&described),
            "{outcome} writes {written:?}, described as {described:?}"
        );
    }
}

#[test]
fn a_category_serialises_to_the_name_the_operator_types() {
    for category in [
        Category::Environment,
        Category::Storage,
        Category::Network,
        Category::Vpn,
        Category::Credentials,
        Category::Services,
        Category::Providers,
        Category::Queue,
        Category::Config,
    ] {
        assert_eq!(
            serde_json::to_string(&category).unwrap_or_default(),
            format!("\"{}\"", category.as_str())
        );
    }
}

#[test]
fn an_overall_serialises_to_a_single_word() {
    for (overall, word) in [
        (Overall::Healthy, "healthy"),
        (Overall::Degraded, "degraded"),
        (Overall::Broken, "broken"),
        (Overall::Unknown, "unknown"),
    ] {
        assert_eq!(
            serde_json::to_string(&overall).unwrap_or_default(),
            format!("\"{word}\"")
        );
    }
}
