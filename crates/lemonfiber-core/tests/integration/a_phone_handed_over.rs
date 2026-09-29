//! Handing somebody's device the way onto the stack, and proving it arrived.
//!
//! The first run issues a code and writes down when; every run asks the media server
//! which of the person's devices are signed in. So the states follow from two facts
//! read fresh — is there an account, is a device signed in — and one remembered: was a
//! code handed over before. A device that signed in and then out reads as it is now.
//!
//! What is held here beside the states: nothing is approved for anybody, no account is
//! made, the code is the address and nothing that signs anybody in, and a server or an
//! address that cannot be reached is said as that rather than as the device's fault.
//!
//! Driven through `dispatch` as every surface reaches it, because the app layer is
//! compiled twice and a path exercised from only one copy leaves the other counted as
//! never run.

use std::path::Path;
use std::sync::Arc;

use crate::common::household::recorded_admin;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::{HandedSession, Handoff, HandoffState};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::http::{Method, Request};
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_ports::docker::{Health, Lifecycle};

/// The owner, who administers the server, and Ana, who has claimed her account.
const HOUSEHOLD: &str = r#"[
    {"Id":"1","Name":"owner","HasPassword":true,"Policy":{"IsAdministrator":true}},
    {"Id":"9","Name":"Ana","HasPassword":true,"Policy":{"IsAdministrator":false}}
]"#;

/// Ana on her phone, and the owner in a browser — whose session is not Ana's.
const ANA_SIGNED_IN: &str = r#"[
    {"UserId":"9","DeviceName":"Pixel 8","Client":"Jellyfin Android",
     "LastActivityDate":"2026-09-29T10:05:00Z"},
    {"UserId":"1","DeviceName":"Firefox","Client":"Jellyfin Web"}
]"#;

/// Only the owner, signed in in a browser.
const NOBODY_ELSE: &str = r#"[{"UserId":"1","DeviceName":"Firefox","Client":"Jellyfin Web"}]"#;

/// The address the household reaches the media server at, as recorded below.
const ADDRESS: &str = "http://192.168.1.20:8096";

/// A media server answering the household, the sessions and the Quick Connect question.
fn a_server(household: Answer, sessions: Answer, quick_connect: Answer) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#)],
        ),
        ("/Sessions", vec![sessions]),
        ("/QuickConnect/Enabled", vec![quick_connect]),
        ("/Users", vec![household]),
    ])
}

/// Everything answering, with `sessions` for who is signed in.
fn answering(sessions: &'static str) -> Arc<Fake> {
    a_server(
        Answer::reply(200, HOUSEHOLD),
        Answer::reply(200, sessions),
        Answer::reply(200, "true"),
    )
}

/// A context over the shipped stack, with the media server up and a password recorded.
fn context(env: &Path, http: Arc<Fake>, host: Option<&str>) -> Ctx {
    lemonfiber_testing::a_context()
        .engine(Arc::new(Reporting::holding(
            &["jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        // A platform that publishes no name of its own, so the address is the one
        // recorded and nothing else.
        .environment(Environment::LinuxNative)
        .settings(Settings {
            env_file: Some(env.to_path_buf()),
            household_host: host.map(str::to_owned),
            ..Settings::default()
        })
        .build()
        .with_http(http)
}

/// What one run said, and everything it sent.
struct Ran {
    /// The hand-off, where the run answered with one.
    handoff: Option<Handoff>,
    /// The code it refused with, where it refused.
    refusal: Option<String>,
    /// Everything that went to the media server.
    sent: Vec<Request>,
}

/// Hand `name`'s device over against `env`, and say what came of it.
async fn handing(env: &Path, name: &str, http: Arc<Fake>, rehearsing: bool) -> Ran {
    let ctx = context(env, http.clone(), Some("192.168.1.20"));
    let ctx = if rehearsing { ctx.rehearsing() } else { ctx };
    ran(&ctx, name, &http).await
}

/// Dispatch the hand-off on `ctx`.
async fn ran(ctx: &Ctx, name: &str, http: &Fake) -> Ran {
    let said = dispatch(
        Command::Handoff {
            name: name.to_owned(),
        },
        ctx,
    )
    .await;
    Ran {
        handoff: match said.as_ref().ok() {
            Some(Outcome::Handoff(handoff)) => Some(handoff.clone()),
            _ => None,
        },
        refusal: said.err().map(|problem| problem.code.as_str().to_owned()),
        sent: http.requests(),
    }
}

/// The hand-off a run answered with.
fn answered(ran: &Ran) -> Handoff {
    ran.handoff
        .clone()
        .unwrap_or_else(|| unreachable!("the run refused: {:?}", ran.refusal))
}

/// Remove a scratch environment and everything beside it.
fn gone(env: &Path) {
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(Path::new("/")));
}

/// Whether anything but a sign-in or a read went to the media server.
fn wrote(sent: &[Request]) -> bool {
    sent.iter().any(|request| {
        request.method != Method::Get && !request.url.contains("/Users/AuthenticateByName")
    })
}

/// The first run issues a code carrying the address, and every app is handed it.
#[tokio::test]
async fn the_first_run_issues_a_code_carrying_the_address() {
    let env = recorded_admin("handoff-issues");
    let ran = handing(&env, "ana", answering(NOBODY_ELSE), false).await;
    let handoff = answered(&ran);
    let kept = std::fs::read_to_string(env.with_file_name("handoffs.json")).unwrap_or_default();
    gone(&env);

    assert_eq!(handoff.state, HandoffState::Ready);
    assert_eq!(
        handoff.name, "Ana",
        "the account's own spelling is the one handed back"
    );
    assert_eq!(handoff.address.as_deref(), Some(ADDRESS));
    assert!(handoff.issued.is_some());
    assert!(
        kept.contains(r#""9""#),
        "the issue was not written down: {kept}"
    );
    assert!(!handoff.clients.is_empty());
    assert!(
        handoff
            .clients
            .iter()
            .all(|client| client.code == ADDRESS && !client.deep_link),
        "a client was handed something other than the address: {:?}",
        handoff.clients
    );
}

/// Run again with no device of theirs signed in, it is waiting, not failed.
#[tokio::test]
async fn a_code_not_yet_used_is_pending() {
    let env = recorded_admin("handoff-pending");
    let first = answered(&handing(&env, "Ana", answering(NOBODY_ELSE), false).await);
    let second = answered(&handing(&env, "Ana", answering(NOBODY_ELSE), false).await);
    gone(&env);

    assert_eq!(second.state, HandoffState::Pending);
    assert_eq!(second.issued, first.issued, "the issue was dated again");
    assert!(second
        .reason
        .is_some_and(|reason| reason.contains("home network")));
}

/// A device of theirs signed in is proved by the media server's own list, and the
/// sessions of anybody else are not counted as theirs.
#[tokio::test]
async fn a_device_signed_in_is_read_from_the_media_server() {
    let env = recorded_admin("handoff-connected");
    let ran = handing(&env, "Ana", answering(ANA_SIGNED_IN), false).await;
    let handoff = answered(&ran);
    gone(&env);

    assert_eq!(handoff.state, HandoffState::Connected);
    assert_eq!(
        handoff.sessions,
        vec![HandedSession {
            device: "Pixel 8".to_owned(),
            client: "Jellyfin Android".to_owned(),
            last_seen: Some("2026-09-29T10:05:00Z".to_owned()),
        }],
        "only Ana's own device is hers"
    );
    assert!(ran
        .sent
        .iter()
        .any(|request| request.url.contains("/Sessions")));
}

/// A device that signed in and then out is not still connected.
#[tokio::test]
async fn a_device_that_signed_out_is_no_longer_connected() {
    let env = recorded_admin("handoff-signed-out");
    let before = answered(&handing(&env, "Ana", answering(ANA_SIGNED_IN), false).await);
    let after = answered(&handing(&env, "Ana", answering(NOBODY_ELSE), false).await);
    gone(&env);

    assert_eq!(before.state, HandoffState::Connected);
    assert_eq!(after.state, HandoffState::Pending);
    assert!(after.sessions.is_empty());
}

/// The sign-in by short code is guided and never approved: nothing is written to the
/// media server at all.
#[tokio::test]
async fn quick_connect_is_guided_and_never_approved() {
    let env = recorded_admin("handoff-quick-connect");
    let ran = handing(&env, "Ana", answering(NOBODY_ELSE), false).await;
    let handoff = answered(&ran);
    gone(&env);

    assert!(handoff.quick_connect);
    assert!(
        handoff
            .steps
            .iter()
            .any(|step| step.contains("Quick Connect") && step.contains("theirs to give")),
        "{:?}",
        handoff.steps
    );
    assert!(!wrote(&ran.sent), "the hand-off wrote to the media server");
    assert!(
        !ran.sent
            .iter()
            .any(|request| request.url.contains("/QuickConnect/Authorize")),
        "a code was approved on somebody's behalf"
    );
}

/// Where the media server does not offer it, the sign-in by code is not described.
#[tokio::test]
async fn quick_connect_is_not_described_where_it_is_off() {
    let env = recorded_admin("handoff-no-quick-connect");
    let http = a_server(
        Answer::reply(200, HOUSEHOLD),
        Answer::reply(200, NOBODY_ELSE),
        Answer::reply(200, "false"),
    );
    let handoff = answered(&handing(&env, "Ana", http, false).await);
    gone(&env);

    assert!(!handoff.quick_connect);
    assert!(!handoff
        .steps
        .iter()
        .any(|step| step.contains("Quick Connect")));
    assert!(!handoff.steps.is_empty());
}

/// A claimed account and an unclaimed one are told to sign in differently.
#[tokio::test]
async fn an_unclaimed_account_is_told_it_has_no_password_yet() {
    let env = recorded_admin("handoff-unclaimed");
    let http = a_server(
        Answer::reply(
            200,
            r#"[{"Id":"9","Name":"Ana","HasPassword":false,"Policy":{"IsAdministrator":false}}]"#,
        ),
        Answer::reply(200, "[]"),
        Answer::reply(200, "true"),
    );
    let unclaimed = answered(&handing(&env, "Ana", http, false).await);
    gone(&env);
    let env = recorded_admin("handoff-claimed");
    let claimed = answered(&handing(&env, "Ana", answering(NOBODY_ELSE), false).await);
    gone(&env);

    assert!(unclaimed
        .steps
        .iter()
        .any(|step| step.contains("password left empty")));
    assert!(claimed
        .steps
        .iter()
        .any(|step| step.contains("their own password")));
}

/// Nobody by that name: not provisioned, the invitation named, and no account made.
#[tokio::test]
async fn nobody_with_an_account_is_unprovisioned_and_none_is_made() {
    let env = recorded_admin("handoff-unprovisioned");
    let ran = handing(&env, "bob", answering(NOBODY_ELSE), false).await;
    let handoff = answered(&ran);
    gone(&env);

    assert_eq!(handoff.state, HandoffState::Unprovisioned);
    assert!(handoff
        .reason
        .is_some_and(|reason| reason.contains("lemonfiber invite bob")));
    assert!(!wrote(&ran.sent), "an account was made on the way");
    assert!(handoff.clients.is_empty());
}

/// Nothing that signs anybody in is in the code: it is the address and no more.
#[tokio::test]
async fn the_code_signs_nobody_in() {
    let env = recorded_admin("handoff-no-credential");
    let handoff = answered(&handing(&env, "Ana", answering(NOBODY_ELSE), false).await);
    gone(&env);

    for client in &handoff.clients {
        assert!(!client.code.contains("token"), "{client:?}");
        assert!(!client.code.contains("minted-earlier"), "{client:?}");
        assert!(!client.code.contains('@'), "{client:?}");
    }
}

/// A rehearsal says what would be handed over and writes nothing down.
#[tokio::test]
async fn a_rehearsal_writes_nothing_down() {
    let env = recorded_admin("handoff-rehearsed");
    let rehearsed = answered(&handing(&env, "Ana", answering(NOBODY_ELSE), true).await);
    let kept = env.with_file_name("handoffs.json").exists();
    let real = answered(&handing(&env, "Ana", answering(NOBODY_ELSE), false).await);
    gone(&env);

    assert!(rehearsed.rehearsed);
    assert_eq!(rehearsed.state, HandoffState::Ready);
    assert!(rehearsed.issued.is_none());
    assert!(!kept, "a rehearsal wrote the issue down");
    assert_eq!(
        real.state,
        HandoffState::Ready,
        "the rehearsal was counted as an issue"
    );
}

/// A media server that does not answer is the server's fault, and said as that.
#[tokio::test]
async fn a_silent_media_server_fails_as_the_server() {
    let env = recorded_admin("handoff-silent");
    let http = a_server(
        Answer::reply(503, ""),
        Answer::reply(503, ""),
        Answer::reply(503, ""),
    );
    let handoff = answered(&handing(&env, "Ana", http, false).await);
    gone(&env);

    assert_eq!(handoff.state, HandoffState::Failed);
    assert!(handoff
        .reason
        .is_some_and(|reason| reason.contains("not anybody's device or app")));
}

/// A server that will not list who is signed in fails as not known, not as the device.
#[tokio::test]
async fn sessions_that_cannot_be_read_fail_without_blaming_the_device() {
    let env = recorded_admin("handoff-sessions-unread");
    let http = a_server(
        Answer::reply(200, HOUSEHOLD),
        Answer::reply(500, ""),
        Answer::reply(200, "true"),
    );
    let handoff = answered(&handing(&env, "Ana", http, false).await);
    let kept = env.with_file_name("handoffs.json").exists();
    gone(&env);

    assert_eq!(handoff.state, HandoffState::Failed);
    assert!(handoff
        .reason
        .is_some_and(|reason| reason.contains("Nothing on their device is at fault")));
    assert!(
        !kept,
        "an issue was written down for a hand-off that failed"
    );
}

/// No address another device could reach is a question of reaching the server.
#[tokio::test]
async fn no_address_is_a_reachability_problem() {
    let env = recorded_admin("handoff-nowhere");
    let http = answering(NOBODY_ELSE);
    let ctx = context(&env, http.clone(), None);
    let handoff = answered(&ran(&ctx, "Ana", &http).await);
    gone(&env);

    assert_eq!(handoff.state, HandoffState::Failed);
    assert!(handoff.address.is_none());
    assert!(handoff
        .reason
        .is_some_and(|reason| reason.contains("reaching the server rather than of any app")));
    assert!(handoff.clients.is_empty());
}

/// The account the program signs in as is never handed over.
#[tokio::test]
async fn the_administrator_is_not_handed_over() {
    let env = recorded_admin("handoff-owner");
    let ran = handing(&env, "owner", answering(NOBODY_ELSE), false).await;
    gone(&env);

    assert_eq!(ran.refusal.as_deref(), Some("HANDOFF-4"));
}

/// A hand-off for nobody is refused before anything is asked.
#[tokio::test]
async fn a_hand_off_for_nobody_is_refused() {
    let env = recorded_admin("handoff-blank");
    let ran = handing(&env, "  ", answering(NOBODY_ELSE), false).await;
    gone(&env);

    assert_eq!(ran.refusal.as_deref(), Some("HANDOFF-1"));
    assert!(ran.sent.is_empty());
}

/// Without the media server's own account there is nothing to ask it as.
#[tokio::test]
async fn nothing_recorded_is_refused_as_not_set_up() {
    let scratch = lemonfiber_fixtures::scratch::Scratch::named("handoff-not-set-up").kept();
    let _ = std::fs::create_dir_all(&scratch);
    let env = scratch.join(".env");
    let ran = handing(&env, "Ana", answering(NOBODY_ELSE), false).await;
    gone(&env);

    assert_eq!(ran.refusal.as_deref(), Some("HANDOFF-3"));
}
