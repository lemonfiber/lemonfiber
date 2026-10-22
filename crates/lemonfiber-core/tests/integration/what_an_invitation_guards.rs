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
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{Invited, Member};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::{stamp, DAY, HOUR};
use lemonfiber_testing::media;

/// The household.
///
/// - `owner` administers the server.
/// - `ana` was reset and then switched off: unclaimed, seen before, and disabled.
/// - `bo` was offered in January and never claimed: an offer nobody took up.
/// - `cy` was a member, reset in January and never claimed again: somebody who has
///   watched on this account.
/// - `ed` was offered yesterday and has not claimed it yet.
/// - `fay` is a member with a password.
fn household() -> Vec<Member> {
    let mut owner = media::member("1", "owner", true);
    owner.access.administrator = true;
    owner.last_seen = None;
    let mut ana = media::member("9", "ana", false);
    ana.access.disabled = true;
    ana.last_seen = Some("2026-03-01T10:00:00Z".to_owned());
    let mut cy = media::member("5", "cy", false);
    cy.last_seen = Some("2025-12-01T10:00:00Z".to_owned());
    let mut fay = media::member("12", "fay", true);
    fay.last_seen = Some(format!("{}Z", stamp(-2 * DAY + 10 * HOUR)));
    vec![
        owner,
        ana,
        media::member("7", "bo", false),
        cy,
        media::member("11", "ed", false),
        fay,
    ]
}

/// When the media server says each invitation was made: the old offers in January, and
/// `ed`'s yesterday.
///
/// January is long past the window, and yesterday is inside it.
fn recorded() -> Vec<Invited> {
    let january = "2026-01-04T09:00:00Z";
    vec![
        Invited {
            member: "7".to_owned(),
            at: january.to_owned(),
        },
        Invited {
            member: "5".to_owned(),
            at: january.to_owned(),
        },
        Invited {
            member: "11".to_owned(),
            at: format!("{}Z", stamp(-DAY + 10 * HOUR)),
        },
    ]
}

/// How the writes that can fail are answered, each on its own.
#[derive(Clone)]
struct Answers {
    /// Putting an account where its person can claim it: the new offer's, or `fay`'s
    /// after a reset.
    readying: Answer,
    /// Taking an account back.
    taking_back: Answer,
    /// Narrowing `ed`, whose offer still stands.
    narrowing: Answer,
    /// When the media server says each invitation was made.
    recorded: Answer,
}

impl Answers {
    /// Every write accepted.
    fn accepted() -> Self {
        Self {
            readying: media::taken(),
            taking_back: media::taken(),
            narrowing: media::taken(),
            recorded: media::answered(&recorded()),
        }
    }
}

/// A media server holding [`household`], answering the writes that can fail with `answers`
/// and every other write with a yes.
fn a_server(answers: Answers) -> Arc<Fake> {
    let mut dee = media::member("4", "dee", false);
    dee.last_seen = None;
    let mut routes = vec![
        (media::identity("when_invited"), vec![answers.recorded]),
        (media::identity("claimable"), vec![answers.readying]),
        (media::identity("withdraw"), vec![answers.taking_back]),
        (media::identity("allow"), vec![answers.narrowing]),
        (media::identity("invite"), vec![media::answered(&dee)]),
    ];
    routes.extend(media::household(&household(), &dee, &recorded()));
    Fake::by_path_in_turn(routes)
}

/// A context over the shipped stack, on the test clock, with the media server moved into
/// its plugin and up.
fn context(env: &std::path::Path, http: Arc<Fake>) -> Ctx {
    media::keyed_in(
        lemonfiber_testing::a_context()
            .engine(Arc::new(media::running()))
            .settings(Settings {
                env_file: Some(env.to_path_buf()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http),
    )
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

    /// The accounts it asked to have taken back.
    fn deleted(&self) -> Vec<String> {
        media::asked_about(&self.sent, "withdraw")
    }

    /// The accounts it asked to have switched off.
    fn suspended(&self) -> Vec<String> {
        media::asked_about(&self.sent, "suspend")
    }

    /// The accounts it asked to have put where their person can claim them.
    fn readied(&self) -> Vec<String> {
        media::asked_about(&self.sent, "claimable")
    }

    /// The accounts it asked to have narrowed.
    fn narrowed(&self) -> Vec<String> {
        media::asked_about(&self.sent, "allow")
    }

    /// Whether a password was taken off anybody.
    fn reset_anybody(&self) -> bool {
        !media::asked(&self.sent, "unclaim").is_empty()
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
        !ran.readied().contains(&"1".to_owned()) && !ran.deleted().contains(&"1".to_owned()),
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
        ran.suspended(),
        ["5"],
        "the reset nobody claimed again was left claimable"
    );
    assert_eq!(
        ran.readied(),
        ["4"],
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
    assert_eq!(
        ran.readied(),
        ["9"],
        "the account was offered and left switched off: {:?}",
        ran.sent
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
    assert!(
        ran.deleted().contains(&"4".to_owned()),
        "{:?}",
        ran.deleted()
    );
    assert!(
        ran.readied().is_empty(),
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
            readying: media::refusing(),
            ..Answers::accepted()
        }),
        nothing,
    )
    .await;

    assert_eq!(ran.refusal(), Some("INVITE-10"));
    assert!(
        ran.deleted().contains(&"4".to_owned()),
        "{:?}",
        ran.deleted()
    );
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
            readying: media::refusing(),
            taking_back: media::refusing(),
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
            narrowing: media::refusing(),
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
    assert_eq!(ran.narrowed(), ["11"]);
    assert!(
        !ran.deleted().contains(&"11".to_owned()),
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
            readying: media::refusing(),
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

    let written: Vec<(&str, String)> = ran
        .sent
        .iter()
        .filter_map(|request| {
            ["unclaim", "claimable"]
                .into_iter()
                .find(|operation| request.url.ends_with(media::identity(operation)))
                .zip(
                    serde_json::from_str::<serde_json::Value>(request.body.as_deref()?)
                        .ok()?
                        .get("id")?
                        .as_str()
                        .map(str::to_owned),
                )
        })
        .collect();
    assert_eq!(
        written,
        [("unclaim", "12".to_owned()), ("claimable", "12".to_owned())],
        "the account was not switched back on after its password came off"
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
            recorded: media::refusing(),
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
    assert_eq!(ran.suspended(), ["5"], "{:?}", ran.sent);
    assert!(ran.readied().is_empty() && ran.narrowed().is_empty());
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
    let sent = http.requests();
    assert!(
        ["withdraw", "suspend", "claimable", "allow"]
            .iter()
            .all(|operation| media::asked(&sent, operation).is_empty()),
        "a rehearsal took something back"
    );
}

/// What could not be taken back is said, because it can still be claimed.
#[tokio::test]
async fn an_invitation_that_could_not_be_taken_back_is_said() {
    let ran = running(
        "read-cannot-take-back",
        Command::Household { member: None },
        a_server(Answers {
            taking_back: media::refusing(),
            ..Answers::accepted()
        }),
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
        format!(
            r#"{{"12":{{"offered":"{}Z","lapses":"2999-01-01T00:00:00Z"}},
            "1":{{"offered":"2026-01-01T10:00:00Z","lapses":"2026-01-03T10:00:00Z"}}}}"#,
            stamp(-DAY + 10 * HOUR)
        ),
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
