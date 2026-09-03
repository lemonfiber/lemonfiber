//! Offering an invitation again once it has run out, driven from outside the crate.
//!
//! **The account is the person.** Everything a household member accumulates hangs off the
//! identifier the media server gave them — what they have watched, and the link the
//! request service holds. So an invitation that has run out has to be offered again on
//! the account that already exists, not on a second one wearing the same name: the two
//! are indistinguishable in every list either appears in, and only one of them is the
//! person anything else in the stack is talking about.
//!
//! What had to change to make that possible is small and was deliberate the other way:
//! the sweep withdraws invitations nobody claimed, withdrawing means removing the
//! account, and the reader that finds "somebody already here" used to skip anybody about
//! to be swept. Offering again therefore meant deleting and rebuilding. It does not now —
//! an invitation is dated by the record of a password moving off the account, and there
//! is one to write whether or not there was a password to take.
//!
//! Driven through `dispatch` as every surface reaches it, because the app layer is
//! compiled twice — once with its in-crate tests and once as the library these binaries
//! link — and a path exercised from only one leaves the other counted as never run.

use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::{Invitation, InvitationStanding};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::ports::Stopped;
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};
use lemonfiber_ports::docker::{Health, Lifecycle};

/// The household: the owner, and two invitations nobody ever claimed.
///
/// Neither has a `LastActivityDate`, which is what makes them offers nobody took up
/// rather than accounts somebody has been in.
const HOUSEHOLD: &str = r#"[
    {"Id":"1","Name":"owner","HasPassword":true,"Policy":{"IsAdministrator":true}},
    {"Id":"9","Name":"Ana","HasPassword":false},
    {"Id":"7","Name":"bo","HasPassword":false}
]"#;

/// Both were made in January, and the clock below is stopped in October — so both are
/// long past the window, and the sweep is about to take them.
const RECORDED: &str = r#"{"Items":[
    {"Type":"UserCreated","Date":"2026-01-04T09:00:00.0000000Z","UserId":"9"},
    {"Type":"UserCreated","Date":"2026-01-04T09:00:00.0000000Z","UserId":"7"}
]}"#;

/// Where Ana's invitation is dated again.
const REDATE: &str = "/Users/9/Password";

/// Where a second account under the same name would be asked for.
const NEW_ACCOUNT: &str = "/Users/New";

/// The stack this repository ships.
fn stack() -> Source {
    Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A scratch environment file holding the media server's recorded password.
fn recorded_admin(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("lemonfiber-renew-{}-{name}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        &["minted", "-earlier"].concat(),
    );
    env
}

/// A media server holding both expired invitations, answering the re-dating with `redate`.
fn a_server(redate: Answer) -> Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in.clone(), signed_in],
        ),
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        ("/System/ActivityLog", vec![Answer::reply(200, RECORDED)]),
        (REDATE, vec![redate]),
        (
            NEW_ACCOUNT,
            vec![Answer::reply(
                200,
                r#"{"Id":"4","Name":"Ana","HasPassword":false}"#,
            )],
        ),
        ("/Users", vec![Answer::reply(200, HOUSEHOLD)]),
    ])
}

/// Everything answering, and the re-dating accepted.
fn answering() -> Arc<Fake> {
    a_server(Answer::reply(204, ""))
}

/// A context over the shipped stack, on a clock stopped well past both invitations.
fn context(env: &std::path::Path, http: Arc<Fake>, rehearsing: bool) -> Ctx {
    let ctx = Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        // Stopped, because everything here turns on a window. Against the real clock the
        // dates above would drift out of the arrangement they were chosen for.
        Stopped::today(),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack(),
        Settings {
            env_file: Some(env.to_path_buf()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(http);
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
    /// The accounts this run asked to have removed.
    fn deleted(&self) -> Vec<&str> {
        self.sent
            .iter()
            .filter(|request| request.method == Method::Delete)
            .filter_map(|request| request.url.rsplit('/').next())
            .collect()
    }

    /// Whether a second account was asked for.
    fn made_another_account(&self) -> bool {
        self.sent
            .iter()
            .any(|request| request.url.contains(NEW_ACCOUNT))
    }

    /// Whether the invitation was dated again.
    fn redated(&self) -> bool {
        self.sent
            .iter()
            .any(|request| request.method == Method::Post && request.url.contains(REDATE))
    }
}

/// Offer somebody an account, and hand back what was said and what was sent.
async fn offering(scratch: &str, name: &str, http: Arc<Fake>, rehearsing: bool) -> Ran {
    let env = recorded_admin(scratch);
    let ctx = context(&env, http.clone(), rehearsing);

    let said = dispatch(
        Command::Invite {
            name: name.to_owned(),
        },
        &ctx,
    )
    .await;

    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    Ran {
        invitation: match said.as_ref().ok() {
            Some(Outcome::Invited(invitation)) => Some(invitation.clone()),
            _ => None,
        },
        refusal: said.err().map(|problem| problem.code.as_str().to_owned()),
        sent: http.requests(),
    }
}

/// The requirement, stated as the three things that must not happen.
///
/// Ana's invitation ran out. Offering it again must leave **her account** in place — not
/// delete it, not build a second one under the same name — and must date the one she has
/// so the window it is offered with is real. All three are asserted together because any
/// one of them alone is satisfied by doing nothing at all.
#[tokio::test]
async fn an_invitation_that_ran_out_is_offered_again_on_the_same_account() {
    let ran = offering("renewed", "ana", answering(), false).await;

    assert!(
        !ran.deleted().contains(&"9"),
        "the account the invitation was for was removed and then rebuilt, so everything \
         hanging off its identifier now points at somebody who does not exist: {:?}",
        ran.deleted()
    );
    assert!(
        !ran.made_another_account(),
        "a second account was made under a name the household already holds"
    );
    assert!(
        ran.redated(),
        "the invitation was offered again without being dated again, so the window it \
         promises ran out before it was sent"
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

/// A rehearsal takes nothing back and dates nothing.
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
    assert!(!ran.redated(), "a rehearsal dated an invitation again");
}

/// A server that will not date it again says so, rather than promising a window.
///
/// The account is untouched either way. What would be wrong is the message: an invitation
/// dated when it was first made has already run out, so the operator would send somebody
/// a window that the next run takes the account away for missing.
#[tokio::test]
async fn an_invitation_that_cannot_be_dated_again_is_refused() {
    let ran = offering("undated", "ana", a_server(Answer::reply(403, "")), false).await;

    assert_eq!(ran.refusal, Some("INVITE-5".to_owned()));
    assert!(
        !ran.made_another_account(),
        "a second account was made after the re-dating was refused"
    );
}
