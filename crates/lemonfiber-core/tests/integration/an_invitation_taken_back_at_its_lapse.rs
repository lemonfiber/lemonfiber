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
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{Lapsed, LeftBecause, Member, TakenBack};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_sidecar::TokenHash;
use lemonfiber_testing::guards::{with_decline, Decline};
use lemonfiber_testing::media;

/// The shipped stack, which runs the decline service, under a scratch directory of its
/// own.
fn stack_with_decline(tag: &str) -> PathBuf {
    let to = lemonfiber_fixtures::scratch::Scratch::named(&format!("lapsed-{tag}")).kept();
    lemonfiber_fixtures::stack::manifest_into(&to);
    to
}

/// A media server holding the owner and `others`, taking every write.
fn holding(others: &[Member]) -> Arc<Fake> {
    let mut owner = media::member("1", "owner", true);
    owner.access.administrator = true;
    let household: Vec<Member> = std::iter::once(owner.clone())
        .chain(others.iter().cloned())
        .collect();
    Fake::by_path_in_turn(media::household(&household, &owner, &[]))
}

/// Ana's account: unclaimed, last seen in March, and switched off where `disabled`.
fn ana_seen(disabled: bool) -> Member {
    let mut ana = media::member("9", "ana", false);
    ana.last_seen = Some("2026-03-01T10:00:00Z".to_owned());
    ana.access.disabled = disabled;
    ana
}

/// Whether `sent` wrote anything about Ana's account.
fn wrote_on_ana(sent: &[Request]) -> bool {
    ["withdraw", "suspend", "claimable", "allow", "unclaim"]
        .iter()
        .any(|operation| media::asked_about(sent, operation).contains(&"9".to_owned()))
}

/// `hours` from now, as the core records a moment.
fn at(hours: i64) -> String {
    jiff::Timestamp::now()
        .checked_add(jiff::SignedDuration::from_hours(hours))
        .map(|moment| moment.strftime("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_default()
}

/// An offer to ana's account `9`, made fifty hours ago and run out two hours ago, and a
/// decline service answering what it did with it at the lapse, where it did anything.
fn offered_and_lapsed(env: &Path, done: Option<TakenBack>) -> Arc<Decline> {
    let record = serde_json::json!({
        "9": {"offered": at(-50), "lapses": at(-2), "decline": TokenHash::of("ana-token").as_str()}
    });
    let _ = std::fs::write(env.with_file_name("invitations.json"), record.to_string());
    let lapses = done
        .map(|outcome| Lapsed {
            token: TokenHash::of("ana-token").as_str().to_owned(),
            account: "9".to_owned(),
            name: "ana".to_owned(),
            issued: 1,
            at: 2,
            outcome,
        })
        .into_iter()
        .collect();
    Arc::new(Decline::default().lapsing(lapses))
}

/// What a household read says, what was sent, and the core's offer record after it.
struct Read {
    report: Option<HouseholdReport>,
    sent: Vec<Request>,
    record: String,
}

async fn read(tag: &str, others: &[Member], lapsed: Option<TakenBack>) -> Read {
    let env = recorded_admin(&format!("lapsed-{tag}"));
    let stack: &'static Path = Box::leak(stack_with_decline(tag).into_boxed_path());
    let decline = offered_and_lapsed(&env, lapsed);
    let http = holding(others);
    let ctx = media::keyed_in(
        lemonfiber_testing::a_context()
            .over(Source::External(stack))
            .engine(Arc::new(media::running()))
            .clock(Arc::new(lemonfiber_adapters::System))
            .settings(Settings {
                env_file: Some(env.clone()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http.clone()),
    );
    let ctx = with_decline(ctx, &decline);

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
    let read = read(
        "switched-off",
        &[ana_seen(true)],
        Some(TakenBack::SwitchedOff),
    )
    .await;

    assert_eq!(ana(read.report.as_ref()), Some(MemberStanding::Expired));
    assert!(!wrote_on_ana(&read.sent));
}

/// An account the decline service removed at its lapse is still listed, as expired, and
/// nothing is said about it having gone; the offer is kept for the next read.
#[tokio::test]
async fn an_account_removed_at_its_lapse_is_listed_as_expired_and_not_as_missing() {
    let read = read("removed", &[], Some(TakenBack::Removed)).await;

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
    for lapsed in [
        None,
        Some(TakenBack::Left {
            why: LeftBecause::Gone,
        }),
    ] {
        let read = read("vanished", &[], lapsed).await;

        assert_eq!(ana(read.report.as_ref()), None);
        assert!(!read.record.contains(r#""9""#), "{}", read.record);
    }
}

/// The core's own sweep reads somebody having been in an account the way the decline
/// service does: an account the media server has seen anybody in is switched off and
/// kept rather than removed.
#[tokio::test]
async fn an_account_somebody_has_been_in_is_switched_off_by_the_sweep_not_removed() {
    let read = read("signed-in-once", &[ana_seen(false)], None).await;

    assert!(media::asked_about(&read.sent, "withdraw").is_empty());
    assert_eq!(media::asked_about(&read.sent, "suspend"), ["9"]);
    assert_eq!(ana(read.report.as_ref()), Some(MemberStanding::Expired));
}
