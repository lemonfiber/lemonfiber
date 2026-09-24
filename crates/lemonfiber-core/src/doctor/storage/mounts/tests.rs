use super::{findings, CHECK};
use crate::doctor::{Finding, Verdict};
use crate::stack::mounts::Crowded;

/// A service that would see the given mounts beneath the data location.
fn crowding(service: &str, mounts: &[&str]) -> Crowded {
    Crowded {
        service: service.to_owned(),
        mounts: mounts.iter().map(|mount| (*mount).to_owned()).collect(),
    }
}

/// The service whose downloads and library are on opposite sides of a boundary.
fn sonarr() -> Crowded {
    crowding(
        "sonarr",
        &[
            "${DATA_ROOT}/downloads:/downloads",
            "${DATA_ROOT}/media:/media",
        ],
    )
}

/// Every word these findings carry — the summary, what it means, each remedy and
/// its detail, and the entries underneath.
///
/// Gathered whole rather than picked out of the verdict, because a match would want
/// an arm for each of the five verdicts and these are two of them. The other three
/// would be lines no run here enters, which the coverage gate counts.
fn said(findings: &[Finding]) -> String {
    format!("{findings:?}")
}

#[test]
fn a_split_data_location_is_reported_rather_than_refused() {
    // The whole point: the stack still runs, and the operator is told what it will
    // cost them.
    let found = findings(&[sonarr()]);
    assert!(
        found
            .iter()
            .all(|finding| matches!(finding.verdict, Verdict::Warn(_))),
        "{found:?}"
    );
}

#[test]
fn what_it_costs_is_named_rather_than_the_rule_it_broke() {
    // "More than one mount beneath the data location" means nothing to most
    // operators. Minutes instead of instants, twice the disk, and nothing left to
    // seed from is what they will actually meet.
    let words = said(&findings(&[sonarr()]));
    assert!(words.contains("copy rather than link"), "{words}");
    assert!(words.contains("twice the disk"), "{words}");
    assert!(words.contains("seed"), "{words}");
}

#[test]
fn the_mounts_it_found_are_shown_rather_than_counted() {
    // A count is not something an operator can act on. The entries are what they
    // will go and edit, so the finding carries them.
    let words = said(&findings(&[sonarr()]));
    assert!(
        words.contains("${DATA_ROOT}/downloads:/downloads"),
        "{words}"
    );
    assert!(words.contains("${DATA_ROOT}/media:/media"), "{words}");
}

#[test]
fn the_choice_can_be_answered_by_the_name_the_finding_carries() {
    // The id offered to `--accept` is the id the finding reports under; a second
    // name written into the sentence would be the one that drifts, and the one
    // nobody could answer with.
    let found = findings(&[sonarr()]);
    let words = said(&found);
    assert!(words.contains(&format!("--accept {CHECK}")), "{words}");
    assert_eq!(
        found.first().map(|finding| finding.check.as_str()),
        Some(CHECK)
    );
}

#[test]
fn each_service_is_reported_as_its_own() {
    // A fork can split the mounts for one service and not for another, and each
    // finding is attributed so the report reads as being about that service.
    let found = findings(&[
        sonarr(),
        crowding("radarr", &["${DATA_ROOT}/a:/a", "${DATA_ROOT}/b:/b"]),
    ]);
    let about: Vec<Option<String>> = found
        .iter()
        .map(|finding| finding.service.clone())
        .collect();
    assert_eq!(
        about,
        vec![Some("sonarr".to_owned()), Some("radarr".to_owned())]
    );
}

#[test]
fn a_stack_that_mounts_it_once_is_told_so_rather_than_left_silent() {
    // The half no probe can answer. That links work on this machine says nothing
    // about what the containers can do, and an operator has no way to tell the two
    // apart unless this says it.
    let found = findings(&[]);
    assert_eq!(found.len(), 1, "{found:?}");
    let words = said(&found);
    assert!(words.contains("link rather than copy"), "{words}");
    assert!(
        found
            .iter()
            .all(|finding| matches!(finding.verdict, Verdict::Pass { .. })),
        "{found:?}"
    );
}
