//! Offering an invitation again once it has run out, driven from outside the crate.
//!
//! **The account is the person.** Everything a household member accumulates hangs off the
//! identifier the media server gave them — what they have watched, and the link the
//! request service holds. So an invitation that has run out has to be offered again on
//! the account that already exists, not on a second one wearing the same name: the two
//! are indistinguishable in every list either appears in, and only one of them is the
//! person anything else in the stack is talking about.
//!
//! The sweep withdraws invitations nobody claimed, and withdrawing means removing the
//! account; offering again does not mean deleting and rebuilding, because an invitation
//! is dated by what this program writes down when it offers one, and the account is
//! readied to be claimed the way a new one is.
//!
//! Driven through `dispatch` as every surface reaches it, because the app layer is
//! compiled twice — once with its in-crate tests and once as the library these binaries
//! link — and a path exercised from only one leaves the other counted as never run.

use std::sync::Arc;

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Allowance, Command, Ctx, Inviting, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::{Invitation, InvitationStanding};
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{Invited, Member};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_testing::media;

/// The household: the owner, and two invitations nobody ever claimed.
///
/// Neither has been seen, which is what makes them offers nobody took up rather than
/// accounts somebody has been in.
fn household() -> Vec<Member> {
    let mut owner = media::member("1", "owner", true);
    owner.access.administrator = true;
    vec![
        owner,
        media::member("9", "Ana", false),
        media::member("7", "bo", false),
    ]
}

/// Both were made in January, and the clock below is stopped in October — so both are
/// long past the window, and the sweep is about to take them.
fn recorded() -> Vec<Invited> {
    ["9", "7"]
        .into_iter()
        .map(|member| Invited {
            member: member.to_owned(),
            at: "2026-01-04T09:00:00Z".to_owned(),
        })
        .collect()
}

/// A media server holding both expired invitations, answering the readying with `ready`.
fn a_server(ready: Answer) -> Arc<Fake> {
    let mut routes = vec![
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        (media::identity("claimable"), vec![ready]),
    ];
    routes.extend(media::household(
        &household(),
        &media::member("4", "Ana", false),
        &recorded(),
    ));
    Fake::by_path_in_turn(routes)
}

/// Everything answering, and the readying accepted.
fn answering() -> Arc<Fake> {
    a_server(media::taken())
}

/// A context over the shipped stack, on a clock stopped well past both invitations.
fn context(env: &std::path::Path, http: Arc<Fake>, rehearsing: bool) -> Ctx {
    // Stopped, because everything here turns on a window. Against the real clock the
    // dates above would drift out of the arrangement they were chosen for.
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
    /// The invitation handed back, where it answered with one.
    invitation: Option<Invitation>,
    /// The code it refused with, where it refused.
    refusal: Option<String>,
    /// Everything that went to the media server.
    sent: Vec<Request>,
}

impl Ran {
    /// The accounts this run asked to have taken back.
    fn deleted(&self) -> Vec<String> {
        media::asked_about(&self.sent, "withdraw")
    }

    /// Whether a second account was asked for.
    fn made_another_account(&self) -> bool {
        !media::asked(&self.sent, "invite").is_empty()
    }

    /// Whether Ana's account was readied to be claimed again.
    fn readied(&self) -> bool {
        media::asked_about(&self.sent, "claimable").contains(&"9".to_owned())
    }
}

/// Offer somebody an account, and hand back what was said and what was sent.
async fn offering(scratch: &str, name: &str, http: Arc<Fake>, rehearsing: bool) -> Ran {
    offering_beside(scratch, name, http, rehearsing, |_| ()).await
}

/// The same, with `beside` given the configuration directory first.
async fn offering_beside(
    scratch: &str,
    name: &str,
    http: Arc<Fake>,
    rehearsing: bool,
    beside: fn(&std::path::Path),
) -> Ran {
    let env = recorded_admin(scratch);
    beside(env.parent().unwrap_or(std::path::Path::new("/")));
    let ctx = context(&env, http.clone(), rehearsing);

    let said = dispatch(
        Command::Invite(Inviting {
            name: name.to_owned(),
            allowance: Allowance::default(),
            confirm: true,
        }),
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

/// The requirement, stated as the three things that must not happen.
///
/// Ana's invitation ran out. Offering it again must leave **her account** in place — not
/// delete it, not build a second one under the same name — and must ready the one she has
/// to be claimed. All three are asserted together because any one of them alone is
/// satisfied by doing nothing at all.
#[tokio::test]
async fn an_invitation_that_ran_out_is_offered_again_on_the_same_account() {
    let ran = offering("renewed", "ana", answering(), false).await;

    assert!(
        !ran.deleted().contains(&"9".to_owned()),
        "the account the invitation was for was removed and then rebuilt, so everything \
         hanging off its identifier now points at somebody who does not exist: {:?}",
        ran.deleted()
    );
    assert!(
        !ran.made_another_account(),
        "a second account was made under a name the household already holds"
    );
    assert!(
        ran.readied(),
        "the invitation was offered again on an account nobody made claimable"
    );
}

/// What comes back names her account, and stands for the full window.
#[tokio::test]
async fn what_comes_back_is_her_own_account_offered_afresh() {
    let ran = offering("afresh", "ana", answering(), false).await;

    let invitation = ran
        .invitation
        .unwrap_or_else(|| unreachable!("offering somebody again answers"));
    assert_eq!(
        invitation.name, "Ana",
        "the operator was handed the name they typed rather than the one she signs in as"
    );
    assert_eq!(
        invitation.standing,
        InvitationStanding::Made,
        "an invitation that had run out was described as one that still stands"
    );
    assert!(invitation.hours > 0);
}

/// Everybody else's expired invitation is still taken back.
///
/// The exception is for the account being offered and for nothing else — otherwise the
/// sweep would have quietly stopped, and invitations nobody claimed would stand for ever.
#[tokio::test]
async fn everybody_else_s_expired_invitation_is_still_taken_back() {
    let ran = offering("others", "ana", answering(), false).await;

    assert_eq!(
        ran.deleted(),
        ["7"],
        "the sweep took back the wrong set: it must take bo's, whose invitation also ran \
         out, and leave Ana's, which is the one being offered again"
    );
    assert_eq!(
        ran.invitation
            .as_ref()
            .map(|invitation| invitation.withdrawn.clone()),
        Some(vec!["bo".to_owned()]),
        "what the operator was told was taken back does not match what was taken"
    );
}

/// A rehearsal takes nothing back and readies nothing.
#[tokio::test]
async fn a_rehearsal_neither_withdraws_nor_dates() {
    let ran = offering("rehearsal", "ana", answering(), true).await;

    assert_eq!(
        ran.invitation
            .as_ref()
            .map(|invitation| invitation.rehearsed),
        Some(true),
        "a rehearsal did not say it was one"
    );
    assert!(ran.deleted().is_empty(), "a rehearsal took an account back");
    assert!(!ran.readied(), "a rehearsal readied an account");
}

/// An offer that cannot be written down says so, rather than promising a window.
///
/// The account is untouched either way. What would be wrong is the message: an invitation
/// nothing dates is one the next run takes back, so the operator would send somebody a
/// window that is not there. A record this program cannot read is one it will not write
/// over, which is how this one is made unwritable.
#[tokio::test]
async fn an_invitation_that_cannot_be_dated_again_is_refused() {
    let ran = offering_beside("undated", "ana", answering(), false, |directory| {
        let _ = std::fs::write(directory.join("invitations.json"), "not a record");
    })
    .await;

    assert_eq!(ran.refusal, Some("INVITE-5".to_owned()));
    assert!(!ran.readied(), "an undated invitation was readied anyway");
    assert!(
        !ran.made_another_account(),
        "a second account was made after the dating was refused"
    );
}

/// A server that will not ready the account says so, rather than sending an address
/// somebody cannot sign in at.
#[tokio::test]
async fn an_account_the_server_will_not_ready_is_refused() {
    let ran = offering("unready", "ana", a_server(media::refusing()), false).await;

    assert_eq!(ran.refusal, Some("INVITE-10".to_owned()));
    assert!(
        !ran.deleted().contains(&"9".to_owned()),
        "an account that was already hers was taken away because it could not be readied"
    );
}
