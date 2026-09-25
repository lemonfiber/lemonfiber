//! What somebody is allowed to watch is chosen while they are being invited, held to
//! the traffic.
//!
//! An account made open and narrowed afterwards is open for as long as it takes anybody
//! to remember, and the person most likely to be given a limit has already been handed
//! the address. So the choice travels with the invitation, and what this file holds is
//! the four things that makes true.
//!
//! **It is sent whole, or the media server undoes the household's own settings.** Driven
//! against `jellyfin/jellyfin:10.10.3`: `POST /Users/{id}/Policy` answers `400` to a body
//! naming only what changed, and *accepts* one carrying the two fields it calls required
//! and nothing else — putting every other field back to its own default. So the account's
//! whole policy is read first and posted back with what was chosen written over it, and
//! this file asserts what was sent rather than that something was.
//!
//! **Nothing is written where nothing was chosen**, and that holds field by field. An
//! offer that named neither says nothing about access at all; one that named only an age
//! limit said nothing about libraries. A value written for what nobody mentioned would
//! widen an account back to every library on its way to narrowing what may be watched on
//! it, behind the household's back.
//!
//! **A refusal never costs an account.** A library nobody holds is settled before the
//! account is made, so what the operator gets is the refusal rather than an open account
//! and a message about it.
//!
//! Driven through `dispatch` rather than asserted about the source, for the reason the
//! invitation's other guards are: a sweep for the words the code uses would pass a policy
//! sent under a different spelling.

use std::sync::Arc;

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Allowance, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::http::Request;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_ports::docker::{Health, Lifecycle};

/// Where what an account may watch is written.
const POLICY: &str = "/Users/9/Policy";

/// Where the account it is written on is read from first.
const ONE_ACCOUNT: &str = "/Users/9";

/// Where an account is asked for.
const NEW_ACCOUNT: &str = "/Users/New";

/// Where the media server's own certificates are read.
const RATINGS: &str = "/Localization/ParentalRatings";

/// What the media server answers there, in one country.
///
/// The shape the pinned image answers with, including the row that carries no age at
/// all — its name for content it has no rating for, which is not a certificate.
const CERTIFICATES: &str = r#"[
    {"Name":"Unrated"},
    {"Name":"U","Value":0},
    {"Name":"12A","Value":12},
    {"Name":"15","Value":15},
    {"Name":"18","Value":18}
]"#;

/// The library list the media server holds, as it names them.
const LIBRARIES: &str = r#"{"Items":[
    {"Id":"db4c17","Name":"Films"},
    {"Id":"a656b9","Name":"Shows"}
]}"#;

/// An account as it opens, with a policy carrying a field this product neither reads
/// nor writes.
///
/// `EnableMediaPlayback` stands for every setting an operator may have changed in the
/// media server's own screens. It is here so that a write which sent only the fields
/// this product knows about fails visibly, which against the real server is the failure
/// that would go unnoticed.
const AS_IT_OPENS: &str = r#"{
    "Id":"9","Name":"ana","HasPassword":false,
    "Policy":{
        "EnableAllFolders":true,
        "EnabledFolders":[],
        "IsAdministrator":false,
        "IsDisabled":false,
        "EnableMediaPlayback":false,
        "AuthenticationProviderId":"Default",
        "PasswordResetProviderId":"Default"
    }
}"#;

/// A media server that signs in, holds nobody, names two libraries and takes a policy.
///
/// The routes are ordered narrowest first, because the fake answers with the first whose
/// fragment the URL contains — so `/Users/9/Policy` has to sit above `/Users/9`, which
/// has to sit above `/Users`.
fn a_server_holding_nobody(policy: &'static str) -> Arc<Fake> {
    a_server(
        policy,
        Answer::reply(200, LIBRARIES),
        Answer::reply(204, ""),
    )
}

/// The same, with what its library list and its policy endpoint answer chosen.
///
/// Both are the two ways this errand can fail against a server that is otherwise
/// working, and each has a different account of what is now true — so each is driven
/// rather than reasoned about.
fn a_server(policy: &'static str, libraries: Answer, written: Answer) -> Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![
                signed_in.clone(),
                signed_in.clone(),
                signed_in.clone(),
                signed_in.clone(),
                signed_in.clone(),
                signed_in,
            ],
        ),
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        // The request service holds no account for anybody, which is nothing to hold
        // rather than a failure to hold something.
        ("/user/jellyfin/", vec![Answer::reply(404, "")]),
        (RATINGS, vec![Answer::reply(200, CERTIFICATES)]),
        (
            "/System/ActivityLog",
            vec![Answer::reply(200, r#"{"Items":[]}"#)],
        ),
        ("/Library/MediaFolders", vec![libraries]),
        (POLICY, vec![written]),
        (
            NEW_ACCOUNT,
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        (ONE_ACCOUNT, vec![Answer::reply(200, policy)]),
        ("/Users", vec![Answer::reply(200, "[]")]),
    ])
}

/// A context over the shipped stack, with the media server up and a password recorded.
fn context(env: &std::path::Path, http: Arc<Fake>) -> Ctx {
    lemonfiber_testing::a_context()
        .engine(Arc::new(Reporting::holding(
            &["jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .clock(Arc::new(lemonfiber_adapters::System))
        .settings(Settings {
            env_file: Some(env.to_path_buf()),
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        })
        .build()
        .with_http(http)
}

/// Offer somebody an account with what they may watch, and hand back everything that
/// was sent doing it.
async fn offering(
    scratch: &str,
    policy: &'static str,
    allowance: Allowance,
) -> (Vec<Request>, Option<Outcome>) {
    driving(scratch, a_server_holding_nobody(policy), allowance).await
}

/// The same, against a server built for this run.
async fn driving(
    scratch: &str,
    http: Arc<Fake>,
    allowance: Allowance,
) -> (Vec<Request>, Option<Outcome>) {
    let env = recorded_admin(scratch);
    let ctx = context(&env, http.clone());

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
            allowance,
            confirm: true,
        },
        &ctx,
    )
    .await;

    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    (http.requests(), made.ok())
}

/// The account the request service holds for somebody who approves their own requests.
///
/// `128` is `AUTO_APPROVE`, read off the pinned image rather than recalled. It is the
/// whole of how a limit on watching and no limit on requesting come apart.
const APPROVES_OWN: &str = r#"{"id":4,"permissions":160}"#;

/// Where what one member may ask for is written.
const PERMISSIONS: &str = "/user/4/settings/permissions";

/// A stack whose request service holds an account that approves its own requests.
fn a_stack_where_requests_arrive_unseen() -> Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![
                signed_in.clone(),
                signed_in.clone(),
                signed_in.clone(),
                signed_in.clone(),
                signed_in.clone(),
                signed_in,
            ],
        ),
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        (
            PERMISSIONS,
            vec![
                Answer::reply(200, APPROVES_OWN),
                Answer::reply(200, APPROVES_OWN),
            ],
        ),
        ("/user/jellyfin/", vec![Answer::reply(200, APPROVES_OWN)]),
        (RATINGS, vec![Answer::reply(200, CERTIFICATES)]),
        (
            "/System/ActivityLog",
            vec![Answer::reply(200, r#"{"Items":[]}"#)],
        ),
        ("/Library/MediaFolders", vec![Answer::reply(200, LIBRARIES)]),
        (POLICY, vec![Answer::reply(204, "")]),
        (
            NEW_ACCOUNT,
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        (ONE_ACCOUNT, vec![Answer::reply(200, AS_IT_OPENS)]),
        ("/Users", vec![Answer::reply(200, "[]")]),
    ])
}

/// What an invitation says about a member being narrowed, for one stack.
fn requesting(made: Option<Outcome>) -> Option<lemonfiber_core::model::Linked> {
    match made {
        Some(Outcome::Invitation(invitation)) => {
            invitation.applied.map(|applied| applied.requesting)
        }
        _ => None,
    }
}

mod asking;
mod watching;
