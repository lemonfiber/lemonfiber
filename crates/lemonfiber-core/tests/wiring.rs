//! The wiring check and its repair, driven through their public surfaces against fakes.
//!
//! Three values decide every case here — what lemonfiber recorded, what the service holds,
//! and what lemonfiber would write — so every test is a different arrangement of those
//! three. Both sides are faked: a filesystem handing back the \*arr's key, and a transport
//! answering as the service would.
//!
//! From here rather than a `#[cfg(test)]` module, as the credentials and releases checks
//! are: the check is built on `#[async_trait]` clients built on another, and in-crate that
//! code is compiled twice with its coverage counted from the copy that never ran.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use common::files::Files;
use common::{Answer, Fake};
use lemonfiber_core::baseline::{Origin, Record};
use lemonfiber_core::doctor::credentials::Target;
use lemonfiber_core::doctor::wiring::{Managed, Wired, WiringCheck, DRIFTED};
use lemonfiber_core::doctor::{Category, Check, Finding, Verdict};
use lemonfiber_core::ports::service::{ClientKind, Credential, DownloadClient};
use lemonfiber_core::repair::{Attempt, Repair, Writing};

/// A Servarr config carrying a generated key, as one reads from disk.
const CONFIG: &str = "<Config><ApiKey>a1b2c3d4e5</ApiKey></Config>";

/// The category lemonfiber files television downloads under.
const OURS: &str = "tv-sonarr";

/// Where Sonarr writes its key, in the stack's bind-mount convention.
fn config() -> PathBuf {
    PathBuf::from("/stack/config/sonarr/config.xml")
}

fn sonarr() -> Target {
    Target {
        id: "sonarr".to_owned(),
        name: "Sonarr".to_owned(),
        base: "http://127.0.0.1:8989".to_owned(),
        config: config(),
        version: 3,
    }
}

/// The download client lemonfiber wires into Sonarr, filing under its own category.
fn want() -> DownloadClient {
    DownloadClient {
        name: "SABnzbd".to_owned(),
        host: "sabnzbd".to_owned(),
        port: 8080,
        kind: ClientKind::Sabnzbd,
        credential: Credential::ApiKey("the-key".to_owned()),
        category: lemonfiber_core::ports::service::Category {
            field: "tvCategory".to_owned(),
            value: OURS.to_owned(),
        },
    }
}

/// What lemonfiber recorded for that client, written or adopted.
fn recorded(value: &str, origin: Origin) -> Record {
    Record {
        value: value.to_owned(),
        at: "1000".to_owned(),
        origin,
    }
}

/// The wiring lemonfiber manages, with whatever it recorded for it.
fn managed(recorded: Option<Record>) -> Managed {
    Managed {
        target: sonarr(),
        clients: vec![Wired {
            want: want(),
            recorded,
        }],
    }
}

/// The service's answer for a client list holding one client at the given category.
fn holding(category: Option<&str>) -> String {
    let field = category.map_or_else(String::new, |value| {
        format!(r#",{{"name":"tvCategory","value":"{value}"}}"#)
    });
    format!(
        r#"[{{"id":7,"fields":[{{"name":"host","value":"sabnzbd"}},{{"name":"port","value":8080}}{field}]}}]"#
    )
}

/// The service's verdict on the client it holds — reachable, or not with its own words.
fn tested(reachable: bool) -> String {
    let failures = if reachable {
        String::new()
    } else {
        r#","validationFailures":[{"errorMessage":"connection refused"}]"#.to_owned()
    };
    format!(r#"[{{"id":7,"isValid":{reachable}{failures}}}]"#)
}

/// A transport answering the client list, and taking an update without complaint.
fn answering(category: Option<&str>) -> Arc<Fake> {
    reaching(category, true)
}

/// The same, carrying the service's own verdict on whether it can reach the client.
///
/// The test route is listed first, because a fake routed by what the URL contains would
/// otherwise answer `downloadclient/testall` with the client list.
fn reaching(category: Option<&str>, reachable: bool) -> Arc<Fake> {
    Fake::by_path(vec![
        (
            "downloadclient/testall",
            Answer::reply(200, tested(reachable)),
        ),
        ("downloadclient", Answer::reply(200, holding(category))),
    ])
}

/// The check over one managed wiring, against the given transport.
fn checking(recorded: Option<Record>, http: Arc<Fake>) -> WiringCheck {
    WiringCheck::new(
        http,
        Files::at(vec![(config(), CONFIG)]),
        vec![managed(recorded)],
        "2000".to_owned(),
    )
}

/// What the check made of one arrangement of the three values.
async fn found(recorded: Option<Record>, holds: Option<&str>) -> Finding {
    let check = checking(recorded, answering(holds));
    let mut findings = check.run().await;
    findings.pop().expect("the check reports on every wiring")
}

/// The service still holds what lemonfiber wrote, and lemonfiber still wants it. Nothing
/// to say beyond that it is where it should be.
#[tokio::test]
async fn a_client_still_filing_where_lemonfiber_wired_it_passes() {
    let finding = found(Some(recorded(OURS, Origin::Written)), Some(OURS)).await;

    assert_eq!(finding.category, Category::Config);
    assert!(matches!(finding.verdict, Verdict::Pass { .. }));
}

/// The operator changed the category, and everything still works. Said, and no more than
/// said: a deliberate edit is not a fault, and a stack that reads degraded because somebody
/// exercised a choice the tool offers them is a stack whose warnings stop meaning anything.
#[tokio::test]
async fn a_category_the_operator_changed_is_noted_rather_than_faulted() {
    let finding = found(Some(recorded(OURS, Origin::Written)), Some("mine")).await;

    let Verdict::Pass { note } = finding.verdict else {
        panic!("an edit that broke nothing is not a fault");
    };
    assert!(
        note.is_some_and(|note| note.contains("the category you set")),
        "the edit is still worth saying out loud"
    );
}

/// The same edit, where the service can no longer reach what it points at. Now it is a
/// fault — not because they changed it, but because nothing downloads through a client the
/// service cannot reach, and an operator watching a queue that never empties is owed the
/// connection between the two.
#[tokio::test]
async fn an_edit_the_service_can_no_longer_reach_is_raised() {
    let check = checking(
        Some(recorded(OURS, Origin::Written)),
        reaching(Some("mine"), false),
    );
    let mut findings = check.run().await;

    let Some(Verdict::Warn(problem)) = findings.pop().map(|found| found.verdict) else {
        panic!("an edit that broke the stack is worth raising");
    };
    assert_eq!(problem.code, DRIFTED);
    assert!(problem.summary.contains("cannot reach"), "{problem:?}");
}

/// A service that has not written its key yet cannot be opened, let alone asked. That is
/// a stack still starting, not a drift — and never a pass either.
#[tokio::test]
async fn a_service_that_has_not_written_its_key_is_unverified() {
    let check = WiringCheck::new(
        answering(Some(OURS)),
        Files::empty(),
        vec![managed(Some(recorded(OURS, Origin::Written)))],
        "2000".to_owned(),
    );
    let mut findings = check.run().await;

    assert!(matches!(
        findings.pop().map(|found| found.verdict),
        Some(Verdict::Unverified { .. })
    ));
}

/// A value the operator set and lemonfiber adopted is theirs, and stays a pass however
/// far lemonfiber's own intent has moved since.
#[tokio::test]
async fn a_value_lemonfiber_adopted_stays_theirs() {
    let finding = found(Some(recorded("mine", Origin::Adopted)), Some("mine")).await;

    assert!(matches!(finding.verdict, Verdict::Pass { .. }));
}

/// Nothing wired there yet is seeding's errand, not a drift — and reporting it here would
/// be a second voice about the same thing.
#[tokio::test]
async fn a_client_not_wired_yet_is_skipped_rather_than_faulted() {
    let check = checking(
        None,
        Fake::by_path(vec![("downloadclient", Answer::reply(200, "[]"))]),
    );
    let mut findings = check.run().await;

    assert!(matches!(
        findings.pop().map(|found| found.verdict),
        Some(Verdict::Skipped { .. })
    ));
}

/// A service that will not answer establishes nothing. Never a pass, and never a drift
/// either — silence is not evidence about the value.
#[tokio::test]
async fn a_service_that_will_not_answer_is_unverified() {
    let check = checking(Some(recorded(OURS, Origin::Written)), Fake::silent());
    let mut findings = check.run().await;

    assert!(matches!(
        findings.pop().map(|found| found.verdict),
        Some(Verdict::Unverified { .. })
    ));
}

/// The mender the check hands back, for the wiring it just reported on.
///
/// Reached through the check rather than built directly, because that is how the runner
/// reaches it: a check that can put right what it found hands back a mender, and a mender
/// nobody could get to would be a repair nobody could ask for.
async fn offering(recorded: Option<Record>, holds: Option<&str>) -> (WiringCheck, Vec<Repair>) {
    let check = checking(recorded, answering(holds));
    let found = check.run().await;
    let repairs = check
        .mender()
        .expect("a wiring can be put right")
        .repairs(&found);
    (check, repairs)
}

/// lemonfiber's own value, fallen behind lemonfiber's intent — so putting it right is
/// restoring lemonfiber's own work, and the repair is offered.
#[tokio::test]
async fn lemonfibers_own_value_fallen_behind_is_offered_and_allowed() {
    let (check, repairs) = offering(
        Some(recorded("old-sonarr", Origin::Written)),
        Some("old-sonarr"),
    )
    .await;
    let repair = repairs.first().expect("a stale wiring is worth offering");
    assert!(
        repair.reversible,
        "the change is journalled, so it can be put back"
    );

    let writing = check
        .mender()
        .expect("a wiring can be put right")
        .may_proceed(repair)
        .await;
    assert_eq!(writing, Writing::Ours);
    assert!(writing.allowed());
}

/// The rule the whole feature turns on. lemonfiber wrote this field once, so reading the
/// baseline's origin alone would call it lemonfiber's to overwrite — and it would write
/// over the operator's own change. Reading what the service holds as well is what stops it.
#[tokio::test]
async fn a_repair_that_would_overwrite_an_operators_change_is_refused() {
    let check = checking(
        Some(recorded(OURS, Origin::Written)),
        reaching(Some("mine"), false),
    );
    let found = check.run().await;
    let repairs = check
        .mender()
        .expect("a wiring can be put right")
        .repairs(&found);
    let repair = repairs
        .first()
        .expect("a broken drift is offered, then refused");

    let writing = check
        .mender()
        .expect("a wiring can be put right")
        .may_proceed(repair)
        .await;

    assert_eq!(writing, Writing::Changed);
    assert!(!writing.allowed());
    assert!(writing
        .refused()
        .is_some_and(|remedy| remedy.detail.is_some_and(|detail| detail.contains("reset"))));
}

/// A service that will not say what it holds cannot establish the value is still
/// lemonfiber's, so nothing is written. Silence is not permission.
#[tokio::test]
async fn a_service_that_will_not_answer_is_not_written_over() {
    let (_, repairs) = offering(
        Some(recorded("old-sonarr", Origin::Written)),
        Some("old-sonarr"),
    )
    .await;
    let repair = repairs.first().expect("a stale wiring is offered");

    let quiet = checking(Some(recorded(OURS, Origin::Written)), Fake::silent());
    let writing = quiet
        .mender()
        .expect("a wiring can be put right")
        .may_proceed(repair)
        .await;

    assert!(!writing.allowed());
}

/// What the repair changed, recorded as it changes it — the category it wrote and the one
/// it wrote over, which is everything a reversal needs to put it back.
#[tokio::test]
async fn a_carried_repair_records_enough_to_be_put_back() {
    let (check, repairs) = offering(
        Some(recorded("old-sonarr", Origin::Written)),
        Some("old-sonarr"),
    )
    .await;
    let repair = repairs.first().expect("a stale wiring is worth offering");

    let attempt = check
        .mender()
        .expect("a wiring can be put right")
        .mend(repair)
        .await;

    let Attempt::Carried { changes } = attempt else {
        panic!("the service took the update");
    };
    let change = changes.first().expect("a configuration change is recorded");
    assert_eq!(change.operation, lemonfiber_core::repair::OPERATION);
    assert_eq!(change.target, "sonarr");
    // Recorded as a change inside the service, not as a setting in lemonfiber's own
    // environment file. Taken for the latter, reversing it would write the field's name
    // into that file and leave Sonarr exactly as it was — and report it restored.
    assert_eq!(
        change.kind,
        lemonfiber_core::journal::Kind::Configured {
            resource: "downloadclient".to_owned(),
            id: "7".to_owned(),
            field: "tvCategory".to_owned(),
            previous: Some("old-sonarr".to_owned()),
            current: OURS.to_owned(),
        }
    );
}

/// A client the service no longer holds is nothing to put back — said plainly, rather
/// than registering a second one under lemonfiber's name.
#[tokio::test]
async fn a_client_the_service_no_longer_holds_is_not_written_afresh() {
    let (_, repairs) = offering(
        Some(recorded("old-sonarr", Origin::Written)),
        Some("old-sonarr"),
    )
    .await;
    let repair = repairs.first().expect("a stale wiring is worth offering");

    let gone = checking(
        Some(recorded("old-sonarr", Origin::Written)),
        Fake::by_path(vec![("downloadclient", Answer::reply(200, "[]"))]),
    );
    let attempt = gone
        .mender()
        .expect("a wiring can be put right")
        .mend(repair)
        .await;

    assert!(matches!(attempt, Attempt::Stopped { .. }));
}
