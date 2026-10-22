//! Making somebody's account claimable again, driven from outside the crate.
//!
//! The mechanism is the invitation's, reused: an account with no password on it is what
//! an invitation *is*, so putting one back to that state is how somebody who lost theirs
//! gets in again without the operator choosing the next one. What comes back is
//! therefore an invitation and not a report of its own — the same address, the same code,
//! the same window, the same line about setting a password. The window is real and is
//! counted **from the reset**: it is written down when the reset is made, so an account
//! offered again is dated by that rather than by when it was made — without which it would
//! be expired the instant it was reset, and switched off by the next invitation offered to
//! anybody.
//!
//! That the operator learns nothing is held structurally in
//! `an_invitation_sets_nobodys_password`, over both commands at once. What is held here
//! is the rest: who it will not do this to, what it says when it cannot, that a
//! rehearsal leaves the password that exists still working — and that the window it is
//! given is the one it was promised, rather than one that ran out before it started.
//!
//! Driven through `dispatch` as every surface reaches it, because the app layer is
//! compiled twice — once with its in-crate tests and once as the library these binaries
//! link — and a path exercised from only one leaves the other counted as never run.

use std::sync::Arc;

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Allowance, Command, Ctx, Inviting, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::{Invitation, InvitationStanding, Linked};
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{Invited, Member};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::{stamp, HOUR};
use lemonfiber_testing::media;

/// The owner, who administers the server.
fn owner() -> Member {
    let mut owner = media::member("1", "owner", true);
    owner.access.administrator = true;
    owner
}

/// The household: the owner, and one ordinary member who has claimed her account. Ana
/// having a password is what makes a reset mean anything.
fn household() -> Vec<Member> {
    vec![owner(), media::member("9", "Ana", true)]
}

/// Ana, reset an hour ago: unclaimed, and seen before.
fn ana_reset() -> Member {
    let mut ana = media::member("9", "Ana", false);
    ana.last_seen = Some(format!("{}Z", stamp(-HOUR)));
    ana
}

/// An invitation the media server records `member` as having been sent at `at`.
fn invited(member: &str, at: &str) -> Invited {
    Invited {
        member: member.to_owned(),
        at: at.to_owned(),
    }
}

/// When the media server says Ana was invited: when her account was made in January, and
/// when her password was taken off two hours ago.
fn ana_invited() -> Vec<Invited> {
    vec![
        invited("9", "2026-01-04T09:00:00Z"),
        invited("9", &format!("{}Z", stamp(-2 * HOUR))),
    ]
}

/// A media server holding `members`, recording `since` as when each was invited, making
/// `made` when asked to invite somebody, and answering a password taken off with `reset`.
fn a_server_holding(members: Answer, since: &[Invited], made: &Member, reset: Answer) -> Arc<Fake> {
    let mut routes = vec![
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        (media::identity("household"), vec![members]),
        (media::identity("unclaim"), vec![reset]),
    ];
    routes.extend(media::household(&[], made, since));
    Fake::by_path_in_turn(routes)
}

/// A media server answering the household with `who`, and the reset with `reset`.
fn a_server(who: Answer, reset: Answer) -> Arc<Fake> {
    a_server_holding(who, &[], &owner(), reset)
}

/// Everything answering: the household is readable and the reset is accepted.
fn answering() -> Arc<Fake> {
    a_server(media::answered(&household()), media::taken())
}

/// A context over the shipped stack, with the media server moved into its plugin and up,
/// and a password recorded.
fn context(env: &std::path::Path, http: Arc<Fake>, rehearsing: bool) -> Ctx {
    let ctx = media::keyed_in(
        lemonfiber_testing::a_context()
            .engine(Arc::new(media::running()))
            .settings(Settings {
                env_file: Some(env.to_path_buf()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http),
    );
    if rehearsing {
        ctx.rehearsing()
    } else {
        ctx
    }
}

/// What one run said, and everything it sent.
struct Ran {
    /// The invitation handed back, where the run answered with one.
    invitation: Option<Invitation>,
    /// The code it refused with, where it refused.
    refusal: Option<String>,
    /// Everything that went to the media server.
    sent: Vec<Request>,
}

impl Ran {
    /// Whether a reset was actually asked for.
    fn reset_somebody(&self) -> bool {
        !media::asked(&self.sent, "unclaim").is_empty()
    }
}

/// Ask for a reset, and hand back what was said and what was sent.
async fn reissuing(scratch: &str, name: &str, http: Arc<Fake>, rehearsing: bool) -> Ran {
    let env = recorded_admin(scratch);
    let ctx = context(&env, http.clone(), rehearsing);

    let said = dispatch(
        Command::Reissue {
            name: name.to_owned(),
        },
        &ctx,
    )
    .await;

    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    Ran {
        invitation: match said.as_ref().ok() {
            Some(Outcome::Invitation(invitation)) => Some(invitation.clone()),
            _ => None,
        },
        refusal: said.err().map(|problem| problem.code.as_str().to_owned()),
        sent: http.requests(),
    }
}

/// The ordinary case: the account is reset, and what comes back is an invitation.
///
/// **`Reset` rather than `Made`**, which is what changes the message: nobody is being
/// invited, and the news is that a password they had has stopped working. The window is
/// the same one an offer carries, because it is the same sweep that acts on it.
#[tokio::test]
async fn a_reset_hands_back_an_invitation_to_send() {
    let ran = reissuing("ordinary", "Ana", answering(), false).await;

    let invitation = ran
        .invitation
        .clone()
        .unwrap_or_else(|| unreachable!("a reset of somebody who is here answers"));
    assert_eq!(invitation.standing, InvitationStanding::Reset);
    assert!(
        invitation.hours > 0,
        "a reset was sent with no window, so nothing will ever act on it"
    );
    assert!(
        invitation.withdrawn.is_empty(),
        "a reset took an invitation back on its way: {:?}",
        invitation.withdrawn
    );
    assert_eq!(
        invitation.linked,
        Linked::NotTried,
        "a reset told the request service something; taking a password away changes \
         nothing it holds"
    );
    assert!(invitation.address.contains("192.168.1.20"));
    assert!(ran.reset_somebody(), "nothing was actually reset");
}

/// The name comes back as the server spells it, not as the operator typed it.
///
/// The same reason the invitation matches without regard to case: what the operator
/// passes on has to be what somebody signs in as, and those differ by capitalisation
/// whenever an account was made from a name typed differently.
#[tokio::test]
async fn the_name_handed_back_is_the_one_they_sign_in_as() {
    let ran = reissuing("case", "ANA", answering(), false).await;

    assert_eq!(
        ran.invitation.map(|invitation| invitation.name),
        Some("Ana".to_owned()),
        "the operator was handed back the name they typed rather than the account's"
    );
}

/// A rehearsal says what would happen and leaves the password that exists working.
///
/// The half worth rehearsing, because a reset cannot be undone: the operator finds out
/// they named the wrong person when that person tells them they are locked out.
#[tokio::test]
async fn a_rehearsal_resets_nothing() {
    let ran = reissuing("rehearsal", "Ana", answering(), true).await;

    assert_eq!(
        ran.invitation
            .as_ref()
            .map(|invitation| invitation.rehearsed),
        Some(true),
        "a rehearsal did not say it was one"
    );
    assert!(
        !ran.reset_somebody(),
        "a rehearsal reset somebody's password: {:?}",
        ran.sent
    );
}

/// The account this program signs in as is not one it will reset.
///
/// Refused here rather than left to the server, which would do it: taking this
/// password away leaves nothing to sign in with, and the next command would fail
/// for a reason that read as unrelated.
#[tokio::test]
async fn the_account_it_signs_in_as_is_refused() {
    let ran = reissuing("owner", "owner", answering(), false).await;

    assert_eq!(ran.refusal, Some("REISSUE-3".to_owned()));
    assert!(
        !ran.reset_somebody(),
        "the administrator's password was reset anyway"
    );
}

/// Nobody by that name is said as such, and nothing is sent.
#[tokio::test]
async fn nobody_by_that_name_is_said_rather_than_guessed_at() {
    let ran = reissuing("stranger", "bo", answering(), false).await;

    assert_eq!(ran.refusal, Some("REISSUE-2".to_owned()));
    assert!(!ran.reset_somebody(), "a stranger's password was reset");
}

/// A household that will not be read stops the command before it picks anybody.
#[tokio::test]
async fn a_household_that_will_not_be_read_resets_nobody() {
    let http = a_server(media::refusing(), media::taken());
    let ran = reissuing("unreadable", "Ana", http, false).await;

    assert_eq!(ran.refusal, Some("REISSUE-1".to_owned()));
    assert!(!ran.reset_somebody(), "somebody was reset unread");
}

/// A server that refuses the reset is reported as having changed nothing.
///
/// The distinction the operator acts on: told it worked, they send an invitation to
/// somebody whose old password still opens the account, and neither of them finds out
/// until it does.
#[tokio::test]
async fn a_refused_reset_is_reported_as_having_changed_nothing() {
    let http = a_server(media::answered(&household()), media::refusing());
    let ran = reissuing("refused", "Ana", http, false).await;

    assert_eq!(ran.refusal, Some("REISSUE-4".to_owned()));
}

/// A reset for nobody is refused for the name rather than by the server.
///
/// Shared with the invitation, which is the point: both need somebody to be for, and
/// one sentence about the name is better than two spellings of the server's `400`.
#[tokio::test]
async fn a_reset_needs_somebody_to_be_for() {
    let ran = reissuing("blank", "   ", answering(), false).await;

    assert_eq!(ran.refusal, Some("INVITE-4".to_owned()));
    assert!(
        ran.sent.is_empty(),
        "a blank name still reached the server: {:?}",
        ran.sent
    );
}

/// A reset account gets the window it was promised, not one that ran out first.
///
/// **The failure this exists to catch destroys an account.** Offering somebody an
/// invitation also withdraws the ones nobody claimed in time, and a reset account is
/// unclaimed. Dated by when the account was *made* — months ago for a household member —
/// it is already past the window the moment it is reset, so the next `invite` withdraws
/// it and takes the watch history with it. Dated by the reset, it stands for the 48 hours
/// the message promised.
///
/// Driven as the two commands actually run, one after the other against one server,
/// because each is correct alone: the reset resets, the sweep sweeps, and the account is
/// gone. Both invitations the media server records for her are here, which is what makes
/// the difference readable — her account made in January, her password taken off at the
/// reset.
#[tokio::test]
async fn a_reset_stands_for_its_own_window_and_not_the_account_s_age() {
    // Ana joined in January and was reset today, so she is unclaimed and her account is
    // years old. Bo is a real expired invitation: made long ago and never claimed.
    let mut since = ana_invited();
    since.push(invited("7", "2026-01-04T09:00:00Z"));
    let http = a_server_holding(
        media::answered(&[owner(), ana_reset(), media::member("7", "bo", false)]),
        &since,
        &media::member("4", "cy", false),
        media::taken(),
    );

    let env = recorded_admin("swept");
    let ctx = context(&env, http.clone(), false);
    let offered = dispatch(
        Command::Invite(Inviting {
            name: "cy".to_owned(),
            allowance: Allowance::default(),
            confirm: true,
        }),
        &ctx,
    )
    .await;
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));

    let withdrawn = match offered.ok() {
        Some(Outcome::Invitation(invitation)) => invitation.withdrawn,
        _ => Vec::new(),
    };
    let sent = http.requests();
    let touched: Vec<String> = ["withdraw", "suspend", "unclaim", "claimable", "allow"]
        .iter()
        .flat_map(|operation| media::asked_about(&sent, operation))
        .filter(|id| id == "9")
        .collect();
    assert!(
        touched.is_empty(),
        "Ana's account was reached for, so her reset was dated by when her account was \
         made: {touched:?}"
    );
    assert_eq!(
        withdrawn,
        ["bo".to_owned()],
        "an empty list means the sweep did not run and this asserts nothing; Ana \
         appearing means her reset was dated by when her account was made, so the window \
         she was told about had already run out before she was told about it"
    );

    let taken = media::asked_about(&sent, "withdraw");
    // Counted before it is judged. The withdrawal asserted above can only have
    // happened through one of these, so an empty list here would mean the two
    // readings disagree — and `all` over an empty list would call that fine.
    assert_eq!(taken.len(), 1, "{taken:?}");
    assert!(
        taken.iter().all(|id| id == "7"),
        "an account other than the expired invitation was deleted: {taken:?}"
    );
}

/// Offering an account to somebody whose password was reset says what it found.
///
/// An operator who forgets they already reset somebody, and reaches for `invite`, gets
/// the same account read back — unclaimed, and easy to describe as a fresh offer. It is
/// not one: they are already in the household, and what they need to hear is that the
/// password they had stopped working. So the offer says what a reset says, because that
/// is what it found.
#[tokio::test]
async fn offering_an_account_to_somebody_reset_says_their_password_went() {
    let http = a_server_holding(
        media::answered(&[ana_reset()]),
        &ana_invited(),
        &owner(),
        media::taken(),
    );

    let env = recorded_admin("offer-after-reset");
    let ctx = context(&env, http.clone(), false);
    let offered = dispatch(
        Command::Invite(Inviting {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
            confirm: true,
        }),
        &ctx,
    )
    .await;
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));

    let invitation = match offered.ok() {
        Some(Outcome::Invitation(invitation)) => Some(invitation),
        _ => None,
    };
    assert_eq!(
        invitation.map(|invitation| invitation.standing),
        Some(InvitationStanding::Reset),
        "somebody already in the household was described as an account nobody has taken up"
    );
    assert!(
        media::asked(&http.requests(), "withdraw").is_empty(),
        "the offer withdrew the account it was describing"
    );
}
