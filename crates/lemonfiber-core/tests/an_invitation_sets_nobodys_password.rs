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
//! Driven through `dispatch` rather than asserted about the source, because a sweep
//! for the words the code uses would pass a password sent under a different spelling
//! and go red on a comment that was only rephrased.

use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::Request;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};
use lemonfiber_ports::docker::{Health, Lifecycle};

/// Where this program signs in as itself, and the only place a password belongs.
const SIGN_IN: &str = "/Users/AuthenticateByName";

/// Where an account is asked for.
const NEW_ACCOUNT: &str = "/Users/New";

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

/// A media server that signs in, holds nobody, and takes the account it is given.
fn answering() -> Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        (
            SIGN_IN,
            vec![signed_in.clone(), signed_in.clone(), signed_in],
        ),
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

/// Offer somebody an account, and hand back everything that was sent doing it.
async fn offering(scratch: &str) -> (Vec<Request>, Option<Outcome>) {
    let env = recorded_admin(scratch);
    let http = answering();
    let ctx = context(&env, http.clone());

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
        },
        &ctx,
    )
    .await;

    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    (http.requests(), made.ok())
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

/// The only password that leaves is this program's own, to its own sign-in.
///
/// Stated as where it *does* go rather than as a filter over where it does not: an
/// exclusion list is a place to add a second entry, and the point of the claim is
/// that there is exactly one.
#[tokio::test]
async fn the_only_password_that_travels_is_this_program_signing_in_as_itself() {
    let (sent, _) = offering("one-password").await;

    let carrying: Vec<String> = sent
        .iter()
        .filter(|request| {
            request
                .body
                .as_ref()
                .is_some_and(|body| body.contains(&admin_password()))
        })
        .map(|request| request.url.clone())
        .collect();

    assert!(
        !carrying.is_empty(),
        "no request carried the recorded credential, so this proves nothing about \
         where one travels — the exchange under it did not run"
    );
    assert!(
        carrying.iter().all(|url| url.contains(SIGN_IN)),
        "a password left for somewhere other than this program's own sign-in: {carrying:?}"
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
