//! Offering somebody an account, driven from outside the crate.
//!
//! The command's own tests live beside it. This drives the same request through the
//! library as every surface links it, because the app layer is compiled twice — once
//! with its in-crate tests and once as the library these binaries link — and a path
//! exercised from only one of those leaves the other copy counted as never run.
//!
//! It also asserts the shape a browser is handed, which is the promise the core
//! makes rather than the terminal's: an invitation that serialised under the wrong
//! name would still print correctly at a prompt.

use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::Linked;
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::{spoke, Reporting, Scripted};
use lemonfiber_ports::docker::{Health, Lifecycle};

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
        "lemonfiber-invitation-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    env
}

/// The accounts the media server already holds, before this invitation.
const HELD: &str = r#"[{"Id":"1","Name":"owner","HasPassword":true}]"#;

/// A media server that signs in and takes the account it is given, beside a request
/// service that answers.
///
/// `linking` is what the request service says to being told about the household, so a
/// test about it being down changes one answer and nothing else.
fn answering_with(linking: Answer) -> Arc<Fake> {
    answering_all(Answer::reply(200, "{}"), linking)
}

/// The same, where the request service's own sign-in answers as the caller says.
///
/// Two ways an outage reaches this: the session is refused, or the import is. They are
/// different calls and the first would leave the second untried, so neither stands in
/// for the other.
fn answering_all(session: Answer, linking: Answer) -> Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in.clone(), signed_in],
        ),
        (
            "/System/ActivityLog",
            vec![Answer::reply(200, r#"{"Items":[]}"#)],
        ),
        (
            "/Users/New",
            vec![Answer::reply(
                200,
                r#"{"Id":"9","Name":"ana","HasPassword":false}"#,
            )],
        ),
        ("/user/import-from-jellyfin", vec![linking]),
        ("/auth/jellyfin", vec![session]),
        ("/Users", vec![Answer::reply(200, HELD)]),
    ])
}

/// A media server and a request service that both answer.
fn answering() -> Arc<Fake> {
    answering_with(Answer::reply(201, "{}"))
}

/// A context over the shipped stack, with the media server up and a password recorded.
fn context(env: &std::path::Path) -> Ctx {
    context_over(env, answering())
}

/// The same, over a transport the caller chose.
fn context_over(env: &std::path::Path, http: Arc<Fake>) -> Ctx {
    context_on(env, http, stack())
}

/// The same again, over a stack the caller chose — for the one case that is about a
/// stack which does not carry a request service at all.
fn context_on(env: &std::path::Path, http: Arc<Fake>, stack: Source) -> Ctx {
    Ctx::new(
        Arc::new(Scripted(Ok(spoke("")))),
        Arc::new(Reporting::holding(
            &["jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack,
        Settings {
            env_file: Some(env.to_path_buf()),
            // Where the household reaches this machine. Without one there is no
            // address to send anybody, which is a refusal rather than an invitation.
            household_host: Some("192.168.1.20".to_owned()),
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(http)
}

#[tokio::test]
async fn an_invitation_carries_one_address_and_the_name_to_sign_in_as() {
    let env = recorded_admin("outside");
    let ctx = context(&env);

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
        },
        &ctx,
    )
    .await;

    let Some(Outcome::Invited(report)) = made.ok() else {
        unreachable!("the invite command answers with an invitation")
    };
    assert_eq!(report.name, "ana");
    assert!(
        report.address.starts_with("http"),
        "an invitation was made with nowhere to send anybody: {}",
        report.address
    );
    assert!(report.hours > 0, "an invitation that stands for no time");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// The request service is told about the whole household, not only the new account.
///
/// It skips anybody it already holds, so naming everybody costs nothing and is what
/// completes a link an earlier run could not make — without anything having been
/// written down in between.
#[tokio::test]
async fn the_request_service_is_told_about_everybody_and_not_only_the_new_account() {
    let env = recorded_admin("linked");
    let http = answering();
    let ctx = context_over(&env, http.clone());

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
        },
        &ctx,
    )
    .await;

    let told = http
        .requests()
        .into_iter()
        .filter(|request| request.url.contains("/user/import-from-jellyfin"))
        .filter_map(|request| request.body)
        .collect::<Vec<String>>();
    assert_eq!(
        told,
        [r#"{"jellyfinUserIds":["1","9"]}"#],
        "the household already here was left out, or told about more than once"
    );
    assert!(
        made.is_ok_and(
            |outcome| matches!(outcome, Outcome::Invited(report) if report.linked == Linked::Made)
        ),
        "the link was made but not reported as made"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// An invitation stands when the request service will not answer.
///
/// The account somebody watches with is the media server's, and it is made whether or
/// not a second service is up. What they cannot do yet is reported; it does not cost
/// them the account, which would be the operator's work thrown away for an outage.
#[tokio::test]
async fn an_invitation_stands_when_the_request_service_will_not_answer() {
    let env = recorded_admin("unlinked");
    let http = answering_with(Answer::reply(503, ""));
    let ctx = context_over(&env, http.clone());

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
        },
        &ctx,
    )
    .await;

    let Some(Outcome::Invited(report)) = made.ok() else {
        unreachable!("a request service that is down is not a reason to refuse an invitation")
    };
    assert_eq!(report.name, "ana", "the account was not made");
    assert_eq!(
        report.linked,
        Linked::NotYet,
        "a link that was not made was reported as though it had been"
    );
    assert!(
        http.requests()
            .iter()
            .any(|request| request.url.contains("/Users/New")),
        "the media-server account was never even attempted"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// Somebody already in the house is named to the request service once, not twice.
///
/// No account is made for them — the one they have is theirs — so the identifier that
/// goes is the one already in the household rather than a second copy of it.
#[tokio::test]
async fn somebody_already_here_is_named_once() {
    let env = recorded_admin("already-here");
    let http = answering();
    let ctx = context_over(&env, http.clone());

    let made = dispatch(
        Command::Invite {
            name: "owner".to_owned(),
        },
        &ctx,
    )
    .await;

    let told = http
        .requests()
        .into_iter()
        .filter(|request| request.url.contains("/user/import-from-jellyfin"))
        .filter_map(|request| request.body)
        .collect::<Vec<String>>();
    assert_eq!(
        told,
        [r#"{"jellyfinUserIds":["1"]}"#],
        "the household already here was named more than once, or not at all"
    );
    assert!(
        !http
            .requests()
            .iter()
            .any(|request| request.url.contains("/Users/New")),
        "a second account was made for somebody who already has one"
    );
    assert!(made.is_ok(), "{made:?}");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A stack with no request service tells nothing, and calls it nothing tried.
///
/// Not `NotYet`: there is no service here that a later run could reach, so saying "not
/// yet" would promise something that is never coming. The household on such a stack
/// watches and does not ask, which is a shape of stack rather than a fault in one.
///
/// **Driven from out here as well as in-crate**, because the app layer is compiled
/// twice — once with its `#[cfg(test)]` modules and once as the library these binaries
/// link — and this path exists only on a stack the shipped manifest is not, so the copy
/// without the in-crate test would otherwise never run it.
#[tokio::test]
async fn a_stack_with_no_request_service_calls_it_nothing_tried() {
    static WITHOUT: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    let dir = WITHOUT.get_or_init(|| {
        let from = std::path::Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/media-stack"
        ));
        let to = std::env::temp_dir().join(format!("lemonfiber-no-asking-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&to);
        let read = std::fs::read_to_string(from.join("stack.toml")).unwrap_or_default();
        // Every block but the request service's, kept in order.
        let kept: String = read
            .split("[[service]]")
            .filter(|block| !block.contains("id = \"seerr\""))
            .collect::<Vec<_>>()
            .join("[[service]]");
        let _ = std::fs::write(to.join("stack.toml"), kept);
        to
    });

    let env = recorded_admin("no-asking");
    let http = answering();
    let ctx = context_on(
        &env,
        http.clone(),
        Source::External(Box::leak(dir.clone().into_boxed_path())),
    );

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
        },
        &ctx,
    )
    .await;

    let Some(Outcome::Invited(report)) = made.ok() else {
        unreachable!("a stack with no request service still makes the account")
    };
    assert_eq!(report.name, "ana", "the account was not made");
    assert_eq!(
        report.linked,
        Linked::NotTried,
        "a stack with no request service was told one might answer later"
    );
    assert!(
        !http
            .requests()
            .iter()
            .any(|request| request.url.contains("/user/import-from-jellyfin")),
        "a stack with no request service was asked to link somebody"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A request service that refuses the session is the same answer as one that refuses
/// the import, reached by a different call.
///
/// Asserted apart because it *is* apart: a refused sign-in leaves the import untried, so
/// a test of only the second would pass over a first that had stopped working.
#[tokio::test]
async fn a_refused_session_leaves_the_invitation_standing_too() {
    let env = recorded_admin("no-session");
    let http = answering_all(Answer::reply(500, ""), Answer::reply(201, "{}"));
    let ctx = context_over(&env, http.clone());

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
        },
        &ctx,
    )
    .await;

    let Some(Outcome::Invited(report)) = made.ok() else {
        unreachable!("a refused session is not a reason to refuse an invitation")
    };
    assert_eq!(report.name, "ana", "the account was not made");
    assert_eq!(report.linked, Linked::NotYet, "{report:?}");
    assert!(
        !http
            .requests()
            .iter()
            .any(|request| request.url.contains("/user/import-from-jellyfin")),
        "the household was named to a service this program had not signed in to"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A rehearsal tells the request service nothing, as it makes no account.
#[tokio::test]
async fn a_rehearsal_tells_the_request_service_nothing() {
    let env = recorded_admin("rehearsed-link");
    let http = answering();
    let ctx = context_over(&env, http.clone()).rehearsing();

    let made = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
        },
        &ctx,
    )
    .await;

    assert!(
        made.is_ok_and(|outcome| matches!(outcome, Outcome::Invited(report) if report.linked == Linked::NotTried)),
        "a rehearsal reported a link it did not make"
    );
    assert!(
        !http
            .requests()
            .iter()
            .any(|request| request.url.contains("/user/import-from-jellyfin")),
        "a rehearsal told the request service about somebody"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// What a browser is handed, which is the core's promise rather than the terminal's.
#[tokio::test]
async fn an_invitation_serialises_under_its_own_name() {
    let env = recorded_admin("json-outside");
    let ctx = context(&env);

    let json = dispatch(
        Command::Invite {
            name: "ana".to_owned(),
        },
        &ctx,
    )
    .await
    .ok()
    .and_then(|outcome| outcome.envelope().to_json())
    .unwrap_or_default();

    assert!(json.contains("\"invitation\""), "{json}");
    assert!(json.contains("ana"), "{json}");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}
