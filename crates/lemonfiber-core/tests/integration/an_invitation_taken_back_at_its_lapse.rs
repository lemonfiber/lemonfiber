//! An invitation the decline service took back when its window closed still reads as one
//! that ran out: a reset it switched off is expired rather than suspended, and an account
//! it removed is still listed, as expired, rather than vanishing as if nobody had been
//! invited. An account that vanished with no lapse recorded for it is not reported.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Command, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::{HouseholdReport, MemberStanding};
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_ports::docker::{Health, Lifecycle};
use lemonfiber_sidecar::decline::{Lapse, Lapses, Left, Outcome as Lapsed, TokenHash};

/// The shipped stack, which runs the decline service, under a scratch directory of its
/// own.
fn stack_with_decline(tag: &str) -> PathBuf {
    let to = lemonfiber_fixtures::scratch::Scratch::named(&format!("lapsed-{tag}")).kept();
    lemonfiber_fixtures::stack::manifest_into(&to);
    to
}

/// A media server holding the owner and `others`, taking every write.
fn holding(others: &str) -> Arc<Fake> {
    let household = format!(
        r#"[{{"Id":"1","Name":"owner","HasPassword":true,"Policy":{{"IsAdministrator":true}}}}{others}]"#
    );
    Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        (
            Method::Get,
            "/System/ActivityLog",
            Answer::reply(200, r#"{"Items":[]}"#),
        ),
        (
            Method::Get,
            "/Users/9",
            Answer::reply(200, r#"{"Id":"9","Policy":{"IsDisabled":false}}"#),
        ),
        (
            Method::Get,
            "/Users",
            Answer::reply(200, Box::leak(household.into_boxed_str())),
        ),
        (Method::Post, "", Answer::reply(204, "")),
        (Method::Delete, "", Answer::reply(204, "")),
        (Method::Get, "", Answer::Silent),
    ])
}

/// `hours` from now, as the core records a moment.
fn at(hours: i64) -> String {
    jiff::Timestamp::now()
        .checked_add(jiff::SignedDuration::from_hours(hours))
        .map(|moment| moment.strftime("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default()
}

/// An offer to ana's account `9`, made fifty hours ago and run out two hours ago, and the
/// decline service's record of what it did with it at the lapse, where it did anything.
fn offered_and_lapsed(env: &Path, stack: &Path, done: Option<Lapsed>) {
    let record = serde_json::json!({
        "9": {"offered": at(-50), "lapses": at(-2), "decline": TokenHash::of("ana-token").as_str()}
    });
    let _ = std::fs::write(env.with_file_name("invitations.json"), record.to_string());
    if let Some(outcome) = done {
        let record_of_lapses = Lapses::default().with(Lapse {
            token: TokenHash::of("ana-token"),
            account: "9".to_owned(),
            name: "ana".to_owned(),
            issued: 1,
            at: 2,
            outcome,
        });
        let _ = std::fs::create_dir_all(stack.join("config/decline"));
        let _ = std::fs::write(
            stack.join("config/decline/lapses.json"),
            record_of_lapses.written(),
        );
    }
}

/// What a household read says, what was sent, and the core's offer record after it.
struct Read {
    report: Option<HouseholdReport>,
    sent: Vec<Request>,
    record: String,
}

async fn read(tag: &str, others: &str, lapsed: Option<Lapsed>) -> Read {
    let env = recorded_admin(&format!("lapsed-{tag}"));
    let stack: &'static Path = Box::leak(stack_with_decline(tag).into_boxed_path());
    offered_and_lapsed(&env, stack, lapsed);
    let http = holding(others);
    let ctx = lemonfiber_testing::a_context()
        .over(Source::External(stack))
        .engine(Arc::new(Reporting::holding(
            &["jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .clock(Arc::new(lemonfiber_adapters::System))
        .settings(Settings {
            env_file: Some(env.clone()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        })
        .build()
        .with_http(http.clone());

    let said = dispatch(Command::Household { member: None }, &ctx).await;
    let record =
        std::fs::read_to_string(env.with_file_name("invitations.json")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(Path::new("/")));
    let _ = std::fs::remove_dir_all(stack);
    Read {
        report: match said {
            Ok(Outcome::Household(report)) => Some(report),
            _ => None,
        },
        sent: http.requests(),
        record,
    }
}

fn ana(report: Option<&HouseholdReport>) -> Option<MemberStanding> {
    report?
        .members
        .iter()
        .find(|member| member.name == "ana")
        .map(|member| member.standing)
}

/// A reset the decline service switched off at its lapse reads as expired, not as an
/// account the operator suspended, and nothing more is written to it.
#[tokio::test]
async fn a_reset_switched_off_at_its_lapse_stands_as_expired() {
    let ana_switched_off = r#",{"Id":"9","Name":"ana","HasPassword":false,
        "LastActivityDate":"2026-03-01T10:00:00.0000000Z","Policy":{"IsDisabled":true}}"#;

    let read = read("switched-off", ana_switched_off, Some(Lapsed::SwitchedOff)).await;

    assert_eq!(ana(read.report.as_ref()), Some(MemberStanding::Expired));
    assert!(!read
        .sent
        .iter()
        .any(|request| request.url.contains("/Users/9")));
}

/// An account the decline service removed at its lapse is still listed, as expired, and
/// nothing is said about it having gone; the offer is kept for the next read.
#[tokio::test]
async fn an_account_removed_at_its_lapse_is_listed_as_expired_and_not_as_missing() {
    let read = read("removed", "", Some(Lapsed::Removed)).await;

    assert_eq!(ana(read.report.as_ref()), Some(MemberStanding::Expired));
    let findings = read
        .report
        .map(|report| report.findings)
        .unwrap_or_default();
    assert!(
        !findings.iter().any(|finding| finding.contains("ana")),
        "{findings:?}"
    );
    assert!(read.record.contains(r#""9""#), "{}", read.record);
}

/// An account that is gone with no removal recorded for its offer is not listed at all,
/// and its offer comes off the record.
#[tokio::test]
async fn an_account_gone_with_no_lapse_recorded_is_not_listed() {
    for lapsed in [None, Some(Lapsed::Left(Left::Gone))] {
        let read = read("vanished", "", lapsed).await;

        assert_eq!(ana(read.report.as_ref()), None);
        assert!(!read.record.contains(r#""9""#), "{}", read.record);
    }
}

/// The core's own sweep reads somebody having been in an account the way the decline
/// service does: a sign-in alone, with no activity since, is enough to switch the
/// account off and keep it rather than remove it.
#[tokio::test]
async fn an_account_somebody_only_signed_in_to_is_switched_off_by_the_sweep_not_removed() {
    let ana_signed_in_once = r#",{"Id":"9","Name":"ana","HasPassword":false,
        "LastLoginDate":"2026-03-01T10:00:00.0000000Z","Policy":{"IsDisabled":false}}"#;

    let read = read("signed-in-once", ana_signed_in_once, None).await;

    assert!(!read
        .sent
        .iter()
        .any(|request| request.method == Method::Delete && request.url.contains("/Users/9")));
    assert!(read
        .sent
        .iter()
        .any(|request| request.method == Method::Post && request.url.contains("/Users/9/Policy")));
    assert_eq!(ana(read.report.as_ref()), Some(MemberStanding::Expired));
}
