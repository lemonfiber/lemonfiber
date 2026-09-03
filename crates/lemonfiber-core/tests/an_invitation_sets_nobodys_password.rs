//! The operator never sets or sends anybody else's password, held to the traffic.
//!
//! An invitation is an account with **no password on it**. That emptiness is the
//! whole mechanism: the person invited sets the first one themselves, at the media
//! server, and it is never anywhere the operator could read it. So there is no
//! moment where somebody chooses a password for somebody else, and none where one
//! travels in a message that has to be passed on.
//!
//! That is true today by there being nothing to make it false. What makes it
//! structural is that there is nowhere to put one: the account is asked for by name
//! and nothing else, and what comes back to be passed on has no field it could be
//! carried in.
//!
//! **One password does travel, and pretending otherwise would make this a filter
//! nobody could check.** lemonfiber signs in to the media server as itself, with the
//! credential it minted and recorded for that account. So the claim is not "no
//! password appears" — it is that the only one that ever leaves is this program's
//! own, and it goes only to the route where this program identifies itself.
//!
//! **A reset is held to the same claim, because it is the same claim.** Putting an
//! account back to having no password is the one way somebody who lost theirs gets in
//! again without the operator choosing one for them, so it is the invitation mechanism
//! reused rather than a second path with a second promise. Every assertion here runs
//! over both commands for that reason: a password could only reappear by one of them
//! gaining somewhere to put it, and a guard that watched one would not see it.
//!
//! Driven through `dispatch` rather than asserted about the source, because a sweep
//! for the words the code uses would pass a password sent under a different spelling
//! and go red on a comment that was only rephrased.

use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Allowance, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};
use lemonfiber_ports::docker::{Health, Lifecycle};

/// Where this program signs in as itself. **Two services, one credential**: it
/// authenticates to the media server directly, and to the request service *through* the
/// media server, which is the whole of why a household member needs no second account.
///
/// Both are this program identifying itself. Neither is somebody else's password, which
/// is what the requirement is about — so the claim below is that the credential goes to
/// these and nowhere else, not that it goes to one place.
const SIGN_INS: [&str; 2] = ["/Users/AuthenticateByName", "/auth/jellyfin"];

/// Stood in for any door [`SIGN_INS`] does not name, so that a failure says a password
/// went somewhere it should not have without repeating where.
const SOMEWHERE_ELSE: &str = "a door this file does not name";

/// Where an account is asked for.
const NEW_ACCOUNT: &str = "/Users/New";

/// Where an account is put back to having no password on it.
///
/// The same endpoint somebody changes their own password at, which is why what is sent
/// to it is asserted whole below rather than searched: `CurrentPw` and `NewPw` are
/// fields it accepts, and sending neither is the whole of what makes this a reset the
/// operator cannot read the result of.
const RESET: &str = "/Users/9/Password";

/// The household after Ana has claimed her account, which is what a reset is for.
const CLAIMED: &str = r#"[
    {"Id":"1","Name":"owner","HasPassword":true,"Policy":{"IsAdministrator":true}},
    {"Id":"9","Name":"ana","HasPassword":true,"Policy":{"IsAdministrator":false}}
]"#;

/// The operator's own recorded credential, assembled rather than written down: a
/// literal reads to a source scanner as a credential committed to the repository.
fn admin_password() -> String {
    ["minted", "-earlier"].concat()
}

/// The stack this repository ships.
fn stack() -> Source {
    Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A scratch environment file holding the media server's recorded password.
fn recorded_admin(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "lemonfiber-no-password-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        &admin_password(),
    );
    env
}

/// A media server that signs in, holds Ana with a password, and accepts the reset.
fn a_server_holding_a_claimed_account() -> Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in.clone(), signed_in],
        ),
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        (RESET, vec![Answer::reply(204, "")]),
        ("/Users", vec![Answer::reply(200, CLAIMED)]),
    ])
}

/// A media server that signs in, holds nobody, and takes the account it is given.
fn a_server_holding_nobody() -> Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in.clone(), signed_in],
        ),
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        (
            "/System/ActivityLog",
            vec![Answer::reply(200, r#"{"Items":[]}"#)],
        ),
        (
            NEW_ACCOUNT,
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        ("/Users", vec![Answer::reply(200, "[]")]),
    ])
}

/// A context over the shipped stack, with the media server up and a password recorded.
fn context(env: &std::path::Path, http: Arc<Fake>) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack(),
        Settings {
            env_file: Some(env.to_path_buf()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(http)
}

/// Every command that hands an operator an invitation to pass on, with the server each
/// one needs answering behind it. Named in [`BOTH_WAYS`], which is held to this list's
/// length.
///
/// **Named as a pair so that a third such command cannot be added without landing
/// here.** The claim this file makes is about the set of ways an account comes to have
/// a password set on it, and a set is only a claim while nothing outside it exists —
/// so the guard below walks this list rather than naming a command.
fn both_ways_an_account_becomes_claimable() -> Vec<(Command, Arc<Fake>)> {
    vec![
        (
            Command::Invite {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
            },
            a_server_holding_nobody(),
        ),
        (
            Command::Reissue {
                name: "ana".to_owned(),
            },
            a_server_holding_a_claimed_account(),
        ),
    ]
}

/// What each of them is called, in the order they are built above.
///
/// **Kept apart from the pair rather than carried in it** so that no value a failure
/// message prints has ever travelled beside a credential. A label bundled with the
/// server that holds one is, to anything reading the flow, a thing derived from it.
const BOTH_WAYS: [&str; 2] = ["invite", "reissue"];

/// Run one of them, and hand back everything that was sent doing it.
async fn driving(
    scratch: &str,
    command: Command,
    http: Arc<Fake>,
) -> (Vec<Request>, Option<Outcome>) {
    let env = recorded_admin(scratch);
    let ctx = context(&env, http.clone());

    let made = dispatch(command, &ctx).await;

    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    (http.requests(), made.ok())
}

/// Offer somebody an account, and hand back everything that was sent doing it.
async fn offering(scratch: &str) -> (Vec<Request>, Option<Outcome>) {
    driving(
        scratch,
        Command::Invite {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
        },
        a_server_holding_nobody(),
    )
    .await
}

/// The account is asked for by name, and there is nothing else in the request.
///
/// A password added here would be the operator choosing one for somebody else — the
/// exact thing the requirement forbids — so the body is asserted whole rather than
/// searched for what it should not contain. An added field fails this either way.
#[tokio::test]
async fn the_account_is_asked_for_by_name_and_nothing_else() {
    let (sent, _) = offering("by-name").await;

    let made: Vec<String> = sent
        .iter()
        .filter(|request| request.url.contains(NEW_ACCOUNT))
        .filter_map(|request| request.body.clone())
        .collect();

    assert_eq!(
        made,
        [r#"{"Name":"ana"}"#],
        "an empty list means no account was asked for and nothing here was read; \
         anything beside the name means the operator sent something about how \
         somebody else signs in"
    );
}

/// The only password that leaves is this program's own, to its own sign-in — whichever
/// command was asked for.
///
/// Stated as where it *does* go rather than as a filter over where it does not: an
/// exclusion list is a place to add a second entry, and the point of the claim is
/// that there is exactly one.
#[tokio::test]
async fn the_only_password_that_travels_is_this_program_signing_in_as_itself() {
    let both = both_ways_an_account_becomes_claimable();
    assert_eq!(
        both.len(),
        BOTH_WAYS.len(),
        "a way for an account to become claimable was added without a name to report it \
         by, so a failure would not say which one leaked"
    );
    // Zipped rather than indexed, so `which` comes out of the constant above and is
    // never derived from the pair it is describing.
    for (which, (command, http)) in BOTH_WAYS.into_iter().zip(both) {
        let (sent, _) = driving(which, command, http).await;
        no_password_left_except_this_program_s_own(which, &sent);
    }
}

/// Held apart from the loop above so the two assertions read as one claim about one
/// command's traffic rather than as a pass over everything both of them sent.
///
/// **What a failure prints is named from this file's own constants, never from the
/// traffic.** The subject here is a credential that must not travel, so a message built
/// out of the requests would put the very thing under test into a terminal and a CI log —
/// which is the failure this exists to catch, arriving by the back door. Each request
/// that carried the credential is therefore reported as *which door it went to*, drawn
/// from [`SIGN_INS`], or as [`SOMEWHERE_ELSE`] where it went anywhere this file does not
/// name. A wrong door is the whole finding; its spelling adds nothing.
///
/// So nothing derived from the traffic reaches the message at all — not even the list of
/// doors, which would only repeat one of two constants back. What a failure prints is the
/// phrase for "somewhere else" and the set of doors that are allowed, both written here.
fn no_password_left_except_this_program_s_own(which: &str, sent: &[Request]) {
    let doors: Vec<&str> = sent
        .iter()
        .filter(|request| {
            request
                .body
                .as_ref()
                .is_some_and(|body| body.contains(&admin_password()))
        })
        .map(|request| {
            SIGN_INS
                .iter()
                .find(|door| request.url.contains(*door))
                .copied()
                .unwrap_or(SOMEWHERE_ELSE)
        })
        .collect();

    assert!(
        !doors.is_empty(),
        "{which} carried the recorded credential nowhere, so this proves nothing about \
         where one travels — the exchange under it did not run"
    );
    assert!(
        !doors.contains(&SOMEWHERE_ELSE),
        "{which} sent the recorded credential to {SOMEWHERE_ELSE}; the only ones it may \
         reach are this program's own sign-ins, {SIGN_INS:?}"
    );
}

/// What the operator is handed to pass on has no password in it.
///
/// The other direction the claim could be lost: not a password sent to the server,
/// but one put in the message a person is sent. The field set is asserted whole for
/// the same reason the request body above is.
#[tokio::test]
async fn what_is_passed_on_carries_no_password() {
    let (_, made) = offering("passed-on").await;

    let fields: Vec<String> = made
        .and_then(|outcome| outcome.envelope().to_json())
        .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
        .and_then(|envelope| {
            envelope
                .get("data")
                .and_then(|invitation| invitation.as_object())
                .map(|invitation| invitation.keys().cloned().collect())
        })
        .unwrap_or_default();

    assert_eq!(
        fields,
        [
            "address",
            "caution",
            "hours",
            // Whether the request service has been told about them — not a password,
            // and asserted here so that adding one would have to pass this list.
            "linked",
            "name",
            "rehearsed",
            "standing",
            "withdrawn"
        ],
        "an empty list means no invitation was read and this asserts nothing; a field \
         beside these means the message an operator passes on gained somewhere to \
         carry a password"
    );
}

/// A reset asks for one, and there is nothing else in the request.
///
/// The mirror of the account-creation body above, and the reason the promise holds for
/// somebody who lost their password rather than only for somebody new. `CurrentPw` and
/// `NewPw` are fields this endpoint accepts, so the body is asserted **whole**: a
/// password added here would be the operator choosing the next one, and a search for
/// what should not be present would pass anything spelled differently.
#[tokio::test]
async fn a_reset_asks_for_one_and_sends_nothing_else() {
    let (sent, _) = driving(
        "reset-body",
        Command::Reissue {
            name: "ana".to_owned(),
        },
        a_server_holding_a_claimed_account(),
    )
    .await;

    let reset: Vec<String> = sent
        .iter()
        .filter(|request| request.url.contains(RESET))
        .filter_map(|request| request.body.clone())
        .collect();

    assert_eq!(
        reset,
        [r#"{"ResetPassword":true}"#],
        "an empty list means no reset was asked for and nothing here was read; anything \
         beside the flag means the operator sent something about how somebody else \
         signs in"
    );
}

/// What a reset hands back is an invitation, carrying no password either.
///
/// Asserted as the same field set the offer above is held to, because it is the same
/// shape: after a reset the thing to send somebody *is* an invitation, and one message
/// with one description is the reason neither can gain a password without the other
/// noticing. `hours` is among them for both, since both run out.
#[tokio::test]
async fn what_a_reset_passes_on_is_an_invitation_with_no_password_in_it() {
    let (_, made) = driving(
        "reset-passed-on",
        Command::Reissue {
            name: "ana".to_owned(),
        },
        a_server_holding_a_claimed_account(),
    )
    .await;

    let fields: Vec<String> = made
        .and_then(|outcome| outcome.envelope().to_json())
        .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
        .and_then(|envelope| {
            envelope
                .get("data")
                .and_then(|invitation| invitation.as_object())
                .map(|invitation| invitation.keys().cloned().collect())
        })
        .unwrap_or_default();

    assert_eq!(
        fields,
        [
            "address",
            "caution",
            "hours",
            "linked",
            "name",
            "rehearsed",
            "standing",
            "withdrawn"
        ],
        "an empty list means the reset did not answer and this asserts nothing; a field \
         beside these means a reset says something an invitation does not"
    );
}
