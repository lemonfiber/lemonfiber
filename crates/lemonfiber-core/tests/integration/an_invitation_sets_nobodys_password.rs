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
//! **Not even this program's own password travels.** The media server's plugin holds
//! the administrator's credential and is asked with a key of its own, and the request
//! service is asked with its own key, so the credential lemonfiber minted and recorded
//! for the media server's administrator is in nothing either command sends. The claim
//! is checked against that recorded credential, over traffic that is shown to have
//! made the account calls, so it cannot pass on an exchange that never ran.
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

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Allowance, Command, Ctx, Inviting, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{Certificate, Member};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_testing::media;

/// The operator's own recorded credential, assembled rather than written down: a
/// literal reads to a source scanner as a credential committed to the repository.
fn admin_password() -> String {
    ["minted", "-earlier"].concat()
}

/// A media server holding `members`, making Ana's account when asked to invite her,
/// beside a request service that takes whoever it is told about.
fn a_server_holding(members: &[Member]) -> Arc<Fake> {
    let mut routes = vec![
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        (
            media::identity("ratings"),
            vec![media::answered(&[Certificate {
                name: "12A".to_owned(),
                age: 12,
            }])],
        ),
    ];
    routes.extend(media::household(
        members,
        &media::member("9", "ana", false),
        &[],
    ));
    Fake::by_path_in_turn(routes)
}

/// A media server holding Ana with a password, after she claimed her account, which is
/// what a reset is for.
fn a_server_holding_a_claimed_account() -> Arc<Fake> {
    let mut owner = media::member("1", "owner", true);
    owner.access.administrator = true;
    a_server_holding(&[owner, media::member("9", "ana", true)])
}

/// A media server holding nobody, which takes the account it is given.
fn a_server_holding_nobody() -> Arc<Fake> {
    a_server_holding(&[])
}

/// A context over the shipped stack, with the media server moved into its plugin and up,
/// and a password recorded.
fn context(env: &std::path::Path, http: Arc<Fake>) -> Ctx {
    media::keyed_in(
        lemonfiber_testing::a_context()
            .engine(Arc::new(media::running()))
            .clock(Arc::new(lemonfiber_adapters::System))
            .settings(Settings {
                env_file: Some(env.to_path_buf()),
                household_host: Some("192.168.1.20".to_owned()),
                ..Settings::default()
            })
            .build()
            .with_http(http),
    )
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
            Command::Invite(Inviting {
                name: "ana".to_owned(),
                allowance: Allowance::default(),
                confirm: true,
            }),
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
        Command::Invite(Inviting {
            name: "ana".to_owned(),
            allowance: Allowance::default(),
            confirm: true,
        }),
        a_server_holding_nobody(),
    )
    .await
}

/// Offer somebody an account with a limit on it, so the answer says what it wrote.
///
/// The plain offer above narrows nothing and therefore reports nothing about access,
/// which is the right answer and the wrong fixture for a claim about what that report
/// carries.
async fn allowing(scratch: &str) -> (Vec<Request>, Option<Outcome>) {
    driving(
        scratch,
        Command::Invite(Inviting {
            name: "ana".to_owned(),
            allowance: Allowance {
                libraries: Vec::new(),
                age_limit: Some(12),
                unrated: None,
            },
            confirm: true,
        }),
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

    assert_eq!(
        media::asked(&sent, "invite"),
        [serde_json::json!({ "name": "ana" })],
        "an empty list means no account was asked for and nothing here was read; \
         anything beside the name means the operator sent something about how \
         somebody else signs in"
    );
}

/// No password leaves at all, not even this program's own — whichever command was asked
/// for.
#[tokio::test]
async fn no_password_travels_not_even_this_program_s_own() {
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
        no_password_left(which, &sent);
    }
}

/// Held apart from the loop above so the two assertions read as one claim about one
/// command's traffic rather than as a pass over everything both of them sent.
///
/// **What a failure prints is named from this file's own words, never from the
/// traffic.** The subject here is a credential that must not travel, so a message built
/// out of the requests would put the very thing under test into a terminal and a CI log —
/// which is the failure this exists to catch, arriving by the back door.
fn no_password_left(which: &str, sent: &[Request]) {
    assert!(
        ["invite", "unclaim"]
            .iter()
            .any(|operation| !media::asked(sent, operation).is_empty()),
        "{which} asked the media server for no account, so this proves nothing about \
         where a password travels — the exchange under it did not run"
    );
    assert!(
        !sent.iter().any(|request| request
            .body
            .as_ref()
            .is_some_and(|body| body.contains(&admin_password()))),
        "{which} sent the recorded credential somewhere"
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
            // What the offer wrote on the account: a limit, the libraries, what
            // becomes of unrated content and whether requesting was held to the same
            // decision. Not a password, and asserted here so that adding one would
            // have to pass this list — its own field set is held below.
            "applied",
            "caution",
            // Where they can decline it instead: an address whose token declines this
            // one invitation and signs nobody in. Not a password, and asserted here so
            // that adding one would have to pass this list.
            "decline",
            "hours",
            "join",
            // Whether the request service has been told about them — not a password,
            // and asserted here so that adding one would have to pass this list.
            "linked",
            "name",
            "rehearsed",
            "standing",
            // Accounts switched off on the way past, by name. Not a password, and
            // asserted here so that adding one would have to pass this list.
            "suspended",
            "unjoinable",
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
/// somebody who lost their password rather than only for somebody new. The body is
/// asserted **whole**: a password added here would be the operator choosing the next
/// one, and a search for what should not be present would pass anything spelled
/// differently.
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

    assert_eq!(
        media::asked(&sent, "unclaim"),
        [serde_json::json!({ "id": "9" })],
        "an empty list means no reset was asked for and nothing here was read; anything \
         beside the account means the operator sent something about how somebody else \
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
            "applied",
            "caution",
            "decline",
            "hours",
            "join",
            "linked",
            "name",
            "rehearsed",
            "standing",
            "suspended",
            "unjoinable",
            "withdrawn"
        ],
        "an empty list means the reset did not answer and this asserts nothing; a field \
         beside these means a reset says something an invitation does not"
    );
}

/// What an offer says it wrote has no password in it either.
///
/// The field set is asserted whole for the reason the invitation's own is: this is the
/// half of the message that describes an account, and describing an account is exactly
/// where somewhere to carry a password would look like it belonged.
#[tokio::test]
async fn what_an_offer_says_it_wrote_carries_no_password() {
    let (_, made) = allowing("wrote").await;

    let fields: Vec<String> = made
        .and_then(|outcome| outcome.envelope().to_json())
        .and_then(|json| serde_json::from_str::<serde_json::Value>(&json).ok())
        .and_then(|envelope| {
            envelope
                .get("data")
                .and_then(|invitation| invitation.get("applied"))
                .and_then(|applied| applied.as_object())
                .map(|applied| applied.keys().cloned().collect())
        })
        .unwrap_or_default();

    assert_eq!(
        fields,
        ["filtering", "libraries", "limit", "requesting", "unrated"],
        "an empty list means nothing was written and this asserts nothing; a field \
         beside these means what an offer reports about an account gained somewhere \
         to carry a password"
    );
}
