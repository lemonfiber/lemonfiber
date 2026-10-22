//! What somebody is allowed to watch is chosen while they are being invited, held to
//! the traffic.
//!
//! An account made open and narrowed afterwards is open for as long as it takes anybody
//! to remember, and the person most likely to be given a limit has already been handed
//! the address. So the choice travels with the invitation: the account is readied to be
//! claimed with what was chosen, in the one call that readies it, and what this file
//! holds is the three things that makes true.
//!
//! **Nothing is chosen where nothing was chosen**, and that holds field by field. An
//! offer that named neither says nothing about access at all; one that named only an age
//! limit says nothing about libraries. A value sent for what nobody mentioned would
//! widen an account back to every library on its way to narrowing what may be watched on
//! it, behind the household's back.
//!
//! **Content nobody rated is held back from anybody narrowed**, unless the operator says
//! otherwise.
//!
//! **A refusal never costs an account.** A library nobody holds is settled before the
//! account is made, so what the operator gets is the refusal rather than an open account
//! and a message about it.
//!
//! Driven through `dispatch` rather than asserted about the source, for the reason the
//! invitation's other guards are: a sweep for the words the code uses would pass an
//! allowance sent under a different spelling.

use std::sync::Arc;

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Allowance, Command, Ctx, Inviting, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::ports::service::{Certificate, Member, NamedLibrary};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_testing::media;

/// The certificates the media server's rating table names, in one country.
fn certificates() -> Vec<Certificate> {
    [("U", 0), ("12A", 12), ("15", 15), ("18", 18)]
        .into_iter()
        .map(|(name, age)| Certificate {
            name: name.to_owned(),
            age,
        })
        .collect()
}

/// The libraries the media server holds, as it names them.
fn libraries() -> Vec<NamedLibrary> {
    [("db4c17", "Films"), ("a656b9", "Shows")]
        .into_iter()
        .map(|(id, name)| NamedLibrary {
            id: id.to_owned(),
            name: name.to_owned(),
        })
        .collect()
}

/// The account an invitation makes for Ana.
fn ana() -> Member {
    media::member("9", "ana", false)
}

/// What the account was readied to be claimed with, each time it was.
fn readied(sent: &[Request]) -> Vec<serde_json::Value> {
    media::asked(sent, "claimable")
        .into_iter()
        .filter_map(|asked| asked.get("allowed").cloned())
        .collect()
}

/// Whether an account was asked for.
fn made_an_account(sent: &[Request]) -> bool {
    !media::asked(sent, "invite").is_empty()
}

/// A media server holding nobody, naming two libraries and readying the account it makes.
fn a_server_holding_nobody() -> Arc<Fake> {
    a_server(media::answered(&libraries()), media::taken())
}

/// The same, with what its library list and the readying of the account answer chosen.
///
/// Both are the two ways this errand can fail against a server that is otherwise
/// working, and each has a different account of what is now true — so each is driven
/// rather than reasoned about.
fn a_server(libraries: Answer, readying: Answer) -> Arc<Fake> {
    let mut routes = vec![
        ("/auth/me", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        // The request service holds no account for anybody, which is nothing to hold
        // rather than a failure to hold something.
        ("/user/jellyfin/", vec![Answer::reply(404, "")]),
    ];
    routes.extend(the_media_server(libraries, readying));
    Fake::by_path_in_turn(routes)
}

/// The media server's half: holding nobody, answering its library list with
/// `libraries`, readying the account it makes for Ana with `readying`, and naming
/// [`certificates`].
fn the_media_server(libraries: Answer, readying: Answer) -> Vec<(&'static str, Vec<Answer>)> {
    let mut routes = vec![
        (media::identity("libraries"), vec![libraries]),
        (
            media::identity("ratings"),
            vec![media::answered(&certificates())],
        ),
        (media::identity("claimable"), vec![readying]),
    ];
    routes.extend(media::household(&[], &ana(), &[]));
    routes
}

/// A context over the shipped stack, with the media server moved into its plugin and up,
/// and a password recorded.
fn context(env: &std::path::Path, http: Arc<Fake>) -> Ctx {
    media::keyed_in(
        lemonfiber_testing::a_context()
            .over(crate::common::household::set_up_beside(env))
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

/// Offer somebody an account with what they may watch, and hand back everything that
/// was sent doing it.
async fn offering(scratch: &str, allowance: Allowance) -> (Vec<Request>, Option<Outcome>) {
    driving(scratch, a_server_holding_nobody(), allowance).await
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
        Command::Invite(Inviting {
            name: "ana".to_owned(),
            allowance,
            confirm: true,
        }),
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
    let mut routes = vec![
        ("/auth/me", vec![Answer::reply(200, "{}")]),
        ("/user/import-from-jellyfin", vec![Answer::reply(201, "{}")]),
        (
            PERMISSIONS,
            vec![
                Answer::reply(200, APPROVES_OWN),
                Answer::reply(200, APPROVES_OWN),
            ],
        ),
        ("/user/jellyfin/", vec![Answer::reply(200, APPROVES_OWN)]),
    ];
    routes.extend(the_media_server(
        media::answered(&libraries()),
        media::taken(),
    ));
    Fake::by_path_in_turn(routes)
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
