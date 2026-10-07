//! What an invitation guards against, driven from outside the crate.
//!
//! An account nobody has claimed is the least guarded thing a household holds: the media
//! server makes it open to every library and bounded by nothing, and whoever signs in to it
//! first sets its password. So every offer is written down with when it runs out, readied
//! with a limit on wrong passwords, and taken back whole where either cannot be done — and
//! a member somebody has watched on is switched off rather than removed when a reset of
//! theirs runs out.
//!
//! Driven through `dispatch` as every surface reaches it, because the app layer is
//! compiled twice — once with its in-crate tests and once as the library these binaries
//! link — and a path exercised from only one leaves the other counted as never run.

use std::sync::Arc;

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Allowance, Command, Ctx, Inviting, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::error::Problem;
use lemonfiber_core::model::{InvitationStanding, MemberStanding};
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_ports::docker::{Health, Lifecycle};

/// The household.
///
/// - `owner` administers the server.
/// - `ana` was reset and then switched off: unclaimed, seen before, and disabled.
/// - `bo` was offered in January and never claimed: an offer nobody took up.
/// - `cy` was a member, reset in January and never claimed again: somebody who has
///   watched on this account.
/// - `ed` was offered yesterday and has not claimed it yet.
/// - `fay` is a member with a password.
const HOUSEHOLD: &str = r#"[
    {"Id":"1","Name":"owner","HasPassword":true,"Policy":{"IsAdministrator":true}},
    {"Id":"9","Name":"ana","HasPassword":false,"LastActivityDate":"2026-03-01T10:00:00.0000000Z",
     "Policy":{"IsDisabled":true}},
    {"Id":"7","Name":"bo","HasPassword":false},
    {"Id":"5","Name":"cy","HasPassword":false,"LastActivityDate":"2025-12-01T10:00:00.0000000Z"},
    {"Id":"11","Name":"ed","HasPassword":false},
    {"Id":"12","Name":"fay","HasPassword":true,"LastActivityDate":"2026-10-04T10:00:00.0000000Z"}
]"#;

/// What the media server recorded: the old offers in January, and `ed`'s yesterday.
///
/// The test clock is stopped at the fourth of October, so January is long past the window
/// and the thirtieth of September is inside it.
const RECORDED: &str = r#"{"Items":[
    {"Type":"UserCreated","Date":"2026-01-04T09:00:00.0000000Z","UserId":"7"},
    {"Type":"UserPasswordChanged","Date":"2026-01-04T09:00:00.0000000Z","UserId":"5"},
    {"Type":"UserCreated","Date":"2026-10-05T10:00:00.0000000Z","UserId":"11"}
]}"#;

/// An account as the media server answers a read of it, for a policy to be written over.
const ACCOUNT: &str = r#"{"Id":"0","Policy":{"IsDisabled":false,"EnableAllFolders":true}}"#;

/// How the writes that can fail are answered, each on its own.
#[derive(Clone)]
struct Answers {
    /// The policy written on the account a new offer makes.
    readying: Answer,
    /// Taking that account back.
    taking_back: Answer,
    /// Narrowing `ed`, whose offer still stands.
    narrowing: Answer,
    /// Switching `fay` back on after a reset.
    switching_on: Answer,
    /// What the media server recorded happening.
    recorded: Answer,
}

impl Answers {
    /// Every write accepted.
    fn accepted() -> Self {
        Self {
            readying: Answer::reply(204, ""),
            taking_back: Answer::reply(204, ""),
            narrowing: Answer::reply(204, ""),
            switching_on: Answer::reply(204, ""),
            recorded: Answer::reply(200, RECORDED),
        }
    }
}

/// A media server holding [`HOUSEHOLD`], answering the writes that can fail with `answers`
/// and every other write with a yes.
fn a_server(answers: Answers) -> Arc<Fake> {
    let yes = Answer::reply(204, "");
    let account = Answer::reply(200, ACCOUNT);
    Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        (Method::Get, "/System/ActivityLog", answers.recorded),
        (
            Method::Post,
            "/Users/New",
            Answer::reply(200, r#"{"Id":"4","Name":"dee","HasPassword":false}"#),
        ),
        (Method::Post, "/Users/4/Policy", answers.readying),
        (Method::Delete, "/Users/4", answers.taking_back),
        (Method::Post, "/Users/11/Policy", answers.narrowing),
        (Method::Post, "/Users/12/Policy", answers.switching_on),
        (Method::Post, "/Policy", yes.clone()),
        (Method::Post, "/Password", yes.clone()),
        (Method::Delete, "/Users/", yes),
        (Method::Get, "/Users/", account),
        (Method::Get, "/Users", Answer::reply(200, HOUSEHOLD)),
        (Method::Get, "", Answer::Silent),
        (Method::Post, "", Answer::Silent),
    ])
}

/// A context over the shipped stack, on the test clock, with the media server up.
fn context(env: &std::path::Path, http: Arc<Fake>) -> Ctx {
    lemonfiber_testing::a_context()
        .engine(Arc::new(Reporting::holding(
            &["jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(Settings {
            env_file: Some(env.to_path_buf()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        })
        .build()
        .with_http(http)
}

/// What one run answered, and everything it sent.
struct Ran {
    /// The outcome, or the refusal.
    said: Result<Outcome, Box<Problem>>,
    /// Everything that went to the media server.
    sent: Vec<Request>,
    /// The record of offers, as the run left it.
    kept: String,
}

impl Ran {
    /// The code it refused with, where it refused.
    fn refusal(&self) -> Option<&str> {
        self.said
            .as_ref()
            .err()
            .map(|problem| problem.code.as_str())
    }

    /// The accounts it asked to have removed.
    fn deleted(&self) -> Vec<&str> {
        self.sent
            .iter()
            .filter(|request| request.method == Method::Delete)
            .filter_map(|request| request.url.rsplit('/').next())
            .collect()
    }

    /// The policies written on one account, as the objects they were sent as.
    fn policies(&self, id: &str) -> Vec<serde_json::Value> {
        let path = format!("/Users/{id}/Policy");
        self.sent
            .iter()
            .filter(|request| request.url.ends_with(&path))
            .filter_map(|request| request.body.as_deref())
            .filter_map(|body| serde_json::from_str(body).ok())
            .collect()
    }

    /// Whether a password was taken off anybody.
    fn reset_anybody(&self) -> bool {
        self.sent
            .iter()
            .any(|request| request.url.ends_with("/Password"))
    }
}

/// Run `command` against `http`, with `beside` given the configuration directory first.
async fn running(
    scratch: &str,
    command: Command,
    http: Arc<Fake>,
    beside: fn(&std::path::Path),
) -> Ran {
    let env = recorded_admin(scratch);
    let directory = env
        .parent()
        .unwrap_or(std::path::Path::new("/"))
        .to_path_buf();
    beside(&directory);
    let said = dispatch(command, &context(&env, http.clone())).await;
    let kept = std::fs::read_to_string(directory.join("invitations.json")).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&directory);
    Ran {
        said,
        sent: http.requests(),
        kept,
    }
}

/// Offer `name` an account, choosing `allowance`.
fn inviting(name: &str, allowance: Allowance) -> Command {
    Command::Invite(Inviting {
        name: name.to_owned(),
        allowance,
        confirm: true,
    })
}

/// Nothing to leave beside the configuration.
fn nothing(_: &std::path::Path) {}

/// A record of offers this program cannot read, and so will not write over.
fn unreadable_record(directory: &std::path::Path) {
    let _ = std::fs::write(directory.join("invitations.json"), "not a record");
}

/// The invitation a run answered with, where it answered with one.
fn invitation(ran: &Ran) -> Option<&lemonfiber_core::model::Invitation> {
    match ran.said.as_ref().ok() {
        Some(Outcome::Invitation(invitation)) => Some(invitation),
        _ => None,
    }
}

/// The administrator is the account this program signs in as, and it is never offered.
#[tokio::test]
async fn the_administrator_is_not_offered() {
    let ran = running(
        "admin",
        inviting(
            "OWNER",
            Allowance {
                age_limit: Some(12),
                ..Allowance::default()
            },
        ),
        a_server(Answers::accepted()),
        nothing,
    )
    .await;

    assert_eq!(ran.refusal(), Some("INVITE-11"));
    assert!(
        ran.policies("1").is_empty() && ran.deleted().is_empty(),
        "the administrator's account was written on: {:?}",
        ran.sent
    );
}

/// A new offer is written down, readied with a limit on wrong passwords, and the sweep it
/// rides along with removes the offer nobody took up and switches off the reset nobody
/// claimed again.
#[tokio::test]
async fn a_reset_nobody_claimed_again_is_switched_off_and_kept() {
    let ran = running(
        "swept",
        inviting("dee", Allowance::default()),
        a_server(Answers::accepted()),
        nothing,
    )
    .await;

    let made = invitation(&ran).unwrap_or_else(|| unreachable!("the offer answers"));
    assert_eq!(made.withdrawn, ["bo".to_owned()]);
    assert_eq!(made.suspended, ["cy".to_owned()]);
    assert_eq!(
        ran.deleted(),
        ["7"],
        "a member somebody has watched on was removed"
    );
    assert_eq!(
        ran.policies("5")
            .first()
            .and_then(|policy| policy.get("IsDisabled").cloned()),
        Some(serde_json::json!(true)),
        "the reset nobody claimed again was left claimable"
    );
    assert_eq!(
        ran.policies("4")
            .first()
            .and_then(|policy| policy.get("LoginAttemptsBeforeLockout").cloned()),
        Some(serde_json::json!(5)),
        "the new account was left open to guessing without end"
    );
    assert!(ran.kept.contains(r#""4""#), "{}", ran.kept);
}

/// A switched-off account offered again is switched back on, and the offer says what it
/// found: a password that stopped working, not a new account.
#[tokio::test]
async fn a_switched_off_account_offered_again_is_switched_back_on() {
    let ran = running(
        "switched-on",
        inviting("ana", Allowance::default()),
        a_server(Answers::accepted()),
        nothing,
    )
    .await;

    assert_eq!(
        invitation(&ran).map(|made| made.standing),
        Some(InvitationStanding::Reset)
    );
    let written = ran.policies("9");
    assert_eq!(
        written
            .first()
            .and_then(|policy| policy.get("IsDisabled").cloned()),
        Some(serde_json::json!(false)),
        "the account was offered and left switched off: {written:?}"
    );
    assert!(ran.kept.contains(r#""9""#), "{}", ran.kept);
}

/// A new account whose offer cannot be written down is taken back rather than left open
/// with nothing to date it.
#[tokio::test]
async fn a_new_account_whose_offer_cannot_be_written_down_is_taken_back() {
    let ran = running(
        "unrecorded",
        inviting("dee", Allowance::default()),
        a_server(Answers::accepted()),
        unreadable_record,
    )
    .await;

    assert_eq!(ran.refusal(), Some("INVITE-9"));
    assert!(ran.deleted().contains(&"4"), "{:?}", ran.deleted());
    assert!(
        ran.policies("4").is_empty(),
        "an account nothing dates was readied anyway"
    );
}

/// A new account the media server will not ready is taken back.
#[tokio::test]
async fn a_new_account_the_server_will_not_ready_is_taken_back() {
    let ran = running(
        "unready",
        inviting("dee", Allowance::default()),
        a_server(Answers {
            readying: Answer::reply(500, ""),
            ..Answers::accepted()
        }),
        nothing,
    )
    .await;

    assert_eq!(ran.refusal(), Some("INVITE-10"));
    assert!(ran.deleted().contains(&"4"), "{:?}", ran.deleted());
}

/// An account that could not be taken back either is named, with what it is.
#[tokio::test]
async fn an_account_that_could_not_be_taken_back_is_named() {
    let ran = running(
        "stranded",
        inviting(
            "dee",
            Allowance {
                age_limit: Some(12),
                ..Allowance::default()
            },
        ),
        a_server(Answers {
            readying: Answer::reply(500, ""),
            taking_back: Answer::reply(500, ""),
            ..Answers::accepted()
        }),
        nothing,
    )
    .await;

    let problem = ran
        .said
        .as_ref()
        .err()
        .unwrap_or_else(|| unreachable!("a refused policy refuses the offer"));
    assert_eq!(problem.code.as_str(), "INVITE-8");
    assert!(
        problem
            .remedies
            .iter()
            .any(|remedy| remedy.action.contains("could not be taken back")),
        "an account left standing was not named: {problem:?}"
    );
}

/// An offer still standing that cannot be narrowed keeps what the account had.
#[tokio::test]
async fn an_offer_still_standing_keeps_what_it_had_when_it_cannot_be_narrowed() {
    let ran = running(
        "unnarrowed",
        inviting(
            "ed",
            Allowance {
                age_limit: Some(12),
                ..Allowance::default()
            },
        ),
        a_server(Answers {
            narrowing: Answer::reply(500, ""),
            ..Answers::accepted()
        }),
        nothing,
    )
    .await;

    let problem = ran
        .said
        .as_ref()
        .err()
        .unwrap_or_else(|| unreachable!("a refused policy refuses the offer"));
    assert_eq!(problem.code.as_str(), "INVITE-8");
    assert!(problem.meaning.contains("still there"), "{problem:?}");
    assert!(
        !ran.deleted().contains(&"11"),
        "an account that was already here was taken away"
    );
}

/// A reset that cannot be written down is refused before the password comes off.
#[tokio::test]
async fn a_reset_that_cannot_be_written_down_leaves_the_password_working() {
    let ran = running(
        "reset-unrecorded",
        Command::Reissue {
            name: "fay".to_owned(),
        },
        a_server(Answers::accepted()),
        unreadable_record,
    )
    .await;

    assert_eq!(ran.refusal(), Some("INVITE-9"));
    assert!(!ran.reset_anybody(), "a password came off an undated reset");
}

/// A reset the media server will not switch back on says so.
#[tokio::test]
async fn a_reset_the_server_will_not_switch_on_says_so() {
    let ran = running(
        "reset-unready",
        Command::Reissue {
            name: "fay".to_owned(),
        },
        a_server(Answers {
            switching_on: Answer::reply(500, ""),
            ..Answers::accepted()
        }),
        nothing,
    )
    .await;

    assert_eq!(ran.refusal(), Some("INVITE-10"));
}

/// A reset switches the account back on after the password comes off, with a limit on
/// wrong passwords, and is written down.
#[tokio::test]
async fn a_reset_switches_the_account_back_on() {
    let ran = running(
        "reset",
        Command::Reissue {
            name: "fay".to_owned(),
        },
        a_server(Answers::accepted()),
        nothing,
    )
    .await;

    let written = ran.policies("12");
    let first = written.first().cloned().unwrap_or_default();
    assert_eq!(first.get("IsDisabled"), Some(&serde_json::json!(false)));
    assert_eq!(
        first.get("InvalidLoginAttemptCount"),
        Some(&serde_json::json!(0))
    );
    assert_eq!(
        first.get("LoginAttemptsBeforeLockout"),
        Some(&serde_json::json!(5))
    );
    assert!(ran.kept.contains(r#""12""#), "{}", ran.kept);
}

/// What the household read says each account stands as.
async fn standings(
    scratch: &str,
    answers: Answers,
) -> (Vec<(String, MemberStanding)>, Vec<String>) {
    let ran = running(
        scratch,
        Command::Household { member: None },
        a_server(answers),
        nothing,
    )
    .await;
    match ran.said {
        Ok(Outcome::Household(report)) => (
            report
                .members
                .into_iter()
                .map(|member| (member.name, member.standing))
                .collect(),
            report.findings,
        ),
        _ => (Vec::new(), Vec::new()),
    }
}

/// Each account says where it stands: a member, an offer out or run out, or switched off.
#[tokio::test]
async fn each_account_says_where_it_stands() {
    let (stood, _) = standings("standings", Answers::accepted()).await;

    assert_eq!(
        stood,
        [
            ("ana".to_owned(), MemberStanding::Suspended),
            ("bo".to_owned(), MemberStanding::Expired),
            ("cy".to_owned(), MemberStanding::Expired),
            ("ed".to_owned(), MemberStanding::Invited),
            ("fay".to_owned(), MemberStanding::Active),
            ("owner".to_owned(), MemberStanding::Active),
        ]
    );
}

/// Where the record of offers cannot be read, nothing is called run out, and it says why.
#[tokio::test]
async fn an_unread_record_calls_nothing_run_out() {
    let (stood, findings) = standings(
        "standings-unread",
        Answers {
            recorded: Answer::reply(500, ""),
            ..Answers::accepted()
        },
    )
    .await;

    assert!(
        stood
            .iter()
            .all(|(_, standing)| *standing != MemberStanding::Expired),
        "{stood:?}"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.contains("could not be read, so none of them")),
        "{findings:?}"
    );
}

/// Reading the household takes back what has run out, the way the next offer would: the
/// offer nobody took up is removed, the reset nobody took up again is switched off, and
/// the one still standing and the one already switched off are left alone.
#[tokio::test]
async fn reading_the_household_takes_back_what_has_run_out() {
    let ran = running(
        "read-takes-back",
        Command::Household { member: None },
        a_server(Answers::accepted()),
        nothing,
    )
    .await;

    assert_eq!(ran.deleted(), ["7"], "{:?}", ran.deleted());
    let switched_off = ran.policies("5");
    assert_eq!(
        switched_off
            .first()
            .and_then(|policy| policy.get("IsDisabled")),
        Some(&serde_json::json!(true)),
        "{switched_off:?}"
    );
    assert!(ran.policies("11").is_empty() && ran.policies("9").is_empty());
}

/// A rehearsed read takes nothing back.
#[tokio::test]
async fn a_rehearsed_read_takes_nothing_back() {
    let env = recorded_admin("read-rehearsed");
    let http = a_server(Answers::accepted());
    let said = dispatch(
        Command::Household { member: None },
        &context(&env, http.clone()).rehearsing(),
    )
    .await;
    if let Some(directory) = env.parent() {
        let _ = std::fs::remove_dir_all(directory);
    }

    assert!(said.is_ok(), "{said:?}");
    assert!(
        http.requests()
            .iter()
            .all(|request| request.method != Method::Delete && !request.url.ends_with("/Policy")),
        "a rehearsal took something back"
    );
}

/// What could not be taken back is said, because it can still be claimed.
#[tokio::test]
async fn an_invitation_that_could_not_be_taken_back_is_said() {
    let http = Fake::by_route(vec![
        (
            Method::Post,
            "/Users/AuthenticateByName",
            Answer::reply(200, r#"{"AccessToken":"token"}"#),
        ),
        (
            Method::Get,
            "/System/ActivityLog",
            Answer::reply(200, RECORDED),
        ),
        (Method::Delete, "/Users/", Answer::reply(500, "")),
        (Method::Get, "/Users/", Answer::reply(200, ACCOUNT)),
        (Method::Post, "/Policy", Answer::reply(204, "")),
        (Method::Get, "/Users", Answer::reply(200, HOUSEHOLD)),
        (Method::Get, "", Answer::Silent),
        (Method::Post, "", Answer::Silent),
    ]);
    let ran = running(
        "read-cannot-take-back",
        Command::Household { member: None },
        http,
        nothing,
    )
    .await;
    let Ok(Outcome::Household(report)) = ran.said else {
        unreachable!("the household was read");
    };

    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding
                .starts_with("bo's invitation ran out and could not be taken back")),
        "{:?}",
        report.findings
    );
    assert!(
        report
            .findings
            .iter()
            .all(|finding| !finding.starts_with("cy's")),
        "{:?}",
        report.findings
    );
}

/// `fay` was offered an account and has claimed it in time; `owner`'s offer ran out and
/// it was claimed only afterwards.
fn offered_to_fay_and_owner(directory: &std::path::Path) {
    let _ = std::fs::write(
        directory.join("invitations.json"),
        r#"{"12":{"offered":"2026-10-05T10:00:00Z","lapses":"2999-01-01T00:00:00Z"},
            "1":{"offered":"2026-01-01T10:00:00Z","lapses":"2026-01-03T10:00:00Z"}}"#,
    );
}

/// Reading the household closes an offer seen taken up in time, and keeps one claimed
/// only after it ran out, which is what keeps that one refused at the door.
#[tokio::test]
async fn reading_the_household_closes_an_offer_taken_up_in_time() {
    let ran = running(
        "read-closes",
        Command::Household { member: None },
        a_server(Answers::accepted()),
        offered_to_fay_and_owner,
    )
    .await;

    assert!(ran.said.is_ok(), "{:?}", ran.refusal());
    assert!(!ran.kept.contains(r#""12""#), "{}", ran.kept);
    assert!(ran.kept.contains(r#""1""#), "{}", ran.kept);
}
