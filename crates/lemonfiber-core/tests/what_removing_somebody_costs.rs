//! Taking somebody out of the household, driven from outside the crate.
//!
//! What an unconfirmed run says is the whole of what a confirmed one does, so most of
//! these assert the *unconfirmed* answer: if it were an estimate rather than a reading,
//! the confirmation it asks for would be worth nothing.
//!
//! Driven through `dispatch` as every surface reaches it, because the app layer is
//! compiled twice — once with its in-crate tests and once as the library these binaries
//! link — and a path exercised from only one leaves the other counted as never run.

use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::{HouseholdRemoval, Revoked};
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
    let dir =
        std::env::temp_dir().join(format!("lemonfiber-removing-{}-{name}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let env = dir.join(".env");
    let _ = lemonfiber_core::config::store::set(
        &env,
        lemonfiber_core::config::JELLYFIN_ADMIN_PASSWORD_KEY,
        "minted-earlier",
    );
    env
}

/// The household: the owner, who administers the server, and one ordinary member.
const HOUSEHOLD: &str = r#"[
    {"Id":"1","Name":"owner","HasPassword":true,"Policy":{"IsAdministrator":true,"EnableAllFolders":true}},
    {"Id":"9","Name":"ana","HasPassword":true,"Policy":{"IsAdministrator":false,"EnableAllFolders":true}}
]"#;

/// Two requests from Ana and one from somebody else, so the count is hers and not the
/// household's.
const REQUESTS: &str = r#"{"pageInfo":{"results":3},"results":[
    {"status":2,"type":"tv","media":{"status":5,"externalServiceId":1},"requestedBy":{"displayName":"ana"}},
    {"status":2,"type":"movie","media":{"status":3,"externalServiceId":2},"requestedBy":{"displayName":"Ana"}},
    {"status":2,"type":"movie","media":{"status":3,"externalServiceId":3},"requestedBy":{"displayName":"bo"}}
]}"#;

/// Everything answering: the media server holds the household, and the request service
/// knows Ana and gives her account up.
fn answering_with(lookup: Answer, gone: Answer) -> Arc<Fake> {
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in],
        ),
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        ("/api/v1/request", vec![Answer::reply(200, REQUESTS)]),
        ("/user/jellyfin/", vec![lookup]),
        ("/api/v1/user/", vec![gone]),
        ("/Users/9", vec![Answer::reply(204, "")]),
        ("/Users", vec![Answer::reply(200, HOUSEHOLD)]),
        ("", vec![Answer::reply(200, "[]")]),
    ])
}

/// The ordinary case: the request service knows them and gives the account up.
fn answering() -> Arc<Fake> {
    answering_with(Answer::reply(200, r#"{"id":7}"#), Answer::reply(200, "{}"))
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
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(http)
}

/// Ask to remove somebody, and hand back what was said and what was sent.
async fn removing(
    scratch: &str,
    name: &str,
    confirm: bool,
    http: Arc<Fake>,
) -> Option<HouseholdRemoval> {
    let env = recorded_admin(scratch);
    let ctx = context(&env, http);
    let said = dispatch(
        Command::Remove {
            name: name.to_owned(),
            confirm,
        },
        &ctx,
    )
    .await;
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    match said.ok() {
        Some(Outcome::Removed(report)) => Some(report),
        _ => None,
    }
}

/// The cost is read before anything is written, and nothing is written.
///
/// The count is **hers**, not the household's: three requests exist and two are Ana's,
/// matched without regard to how the request service capitalised her name.
#[tokio::test]
async fn what_it_costs_is_said_without_removing_anybody() {
    let http = answering();
    let said = removing("preview", "ana", false, http.clone()).await;

    let report = said.unwrap_or_else(|| unreachable!("an unconfirmed removal still answers"));
    assert!(!report.confirmed);
    assert_eq!(report.requests, 2, "the count was not hers alone");
    assert!(report.asks_through_the_request_service);
    assert_eq!(report.revoked, Revoked::Nothing);

    let sent = http.requests();
    assert!(
        !sent.iter().any(|request| {
            matches!(request.method, lemonfiber_core::ports::http::Method::Delete)
        }),
        "an unconfirmed removal deleted something"
    );
}

/// Confirmed, it revokes in both places — the media server first.
///
/// The order is the claim: the request service authenticates *through* the media server,
/// so taking that one first means a failure afterwards leaves an account nobody can sign
/// in to rather than somebody who can still watch.
#[tokio::test]
async fn confirmed_it_revokes_in_both_places_and_the_media_server_first() {
    let http = answering();
    let said = removing("confirmed", "ana", true, http.clone()).await;

    let report = said.unwrap_or_else(|| unreachable!("a confirmed removal answers"));
    assert!(report.confirmed);
    assert_eq!(report.revoked, Revoked::Everywhere);

    let deletes: Vec<String> = http
        .requests()
        .into_iter()
        .filter(|request| matches!(request.method, lemonfiber_core::ports::http::Method::Delete))
        .map(|request| request.url)
        .collect();
    assert_eq!(
        deletes.len(),
        2,
        "both accounts were not taken: {deletes:?}"
    );
    assert!(
        deletes.first().is_some_and(|url| url.contains("/Users/9")),
        "the request service was asked first: {deletes:?}"
    );
    assert!(
        deletes
            .get(1)
            .is_some_and(|url| url.contains("/api/v1/user/7")),
        "{deletes:?}"
    );
}

/// Somebody the request service never heard of costs nothing there, and is not a failure.
#[tokio::test]
async fn somebody_who_never_asked_for_anything_has_nothing_to_revoke_there() {
    let http = answering_with(Answer::reply(404, ""), Answer::reply(200, "{}"));
    let said = removing("never-asked", "ana", true, http.clone()).await;

    let report = said.unwrap_or_else(|| unreachable!("a confirmed removal answers"));
    assert!(!report.asks_through_the_request_service);
    assert_eq!(
        report.revoked,
        Revoked::Everywhere,
        "an account that was never there was reported as one left behind"
    );
    assert!(
        !http
            .requests()
            .iter()
            .any(|request| request.url.contains("/api/v1/user/")
                && matches!(request.method, lemonfiber_core::ports::http::Method::Delete)),
        "the request service was asked to remove an account it does not hold"
    );
}

/// A request service that will not give the account up leaves it, and says so.
#[tokio::test]
async fn an_account_the_request_service_keeps_is_reported_rather_than_claimed_gone() {
    let http = answering_with(Answer::reply(200, r#"{"id":7}"#), Answer::reply(500, ""));
    let said = removing("kept", "ana", true, http).await;

    let report = said.unwrap_or_else(|| unreachable!("a confirmed removal answers"));
    assert_eq!(report.revoked, Revoked::MediaServerOnly);
    assert!(
        report
            .findings
            .iter()
            .any(|finding| finding.contains("still holds an account")),
        "{report:?}"
    );
}

/// The account this program signs in as is refused, by this program.
///
/// The media server refuses it too, but with a `400` where another administrator remains
/// and a `500` where it is the last account — neither of which is a sentence anybody can
/// act on, and both of which would arrive after the operator had already confirmed.
#[tokio::test]
async fn the_account_that_runs_the_server_is_refused_here_rather_than_by_the_server() {
    let http = answering();
    let env = recorded_admin("owner");
    let ctx = context(&env, http.clone());

    let refused = dispatch(
        Command::Remove {
            name: "owner".to_owned(),
            confirm: true,
        },
        &ctx,
    )
    .await;

    assert!(
        refused.is_err(),
        "the account running the server was removed"
    );
    assert!(
        !http
            .requests()
            .iter()
            .any(|request| matches!(request.method, lemonfiber_core::ports::http::Method::Delete)),
        "it was refused only after asking the server to do it"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// Nobody by that name is a refusal that names what to do, not an empty removal.
#[tokio::test]
async fn nobody_by_that_name_is_refused_rather_than_removed_silently() {
    let http = answering();
    let env = recorded_admin("nobody");
    let ctx = context(&env, http.clone());

    let refused = dispatch(
        Command::Remove {
            name: "  ".to_owned(),
            confirm: true,
        },
        &ctx,
    )
    .await;
    assert!(refused.is_err(), "a blank name removed somebody");

    let missing = dispatch(
        Command::Remove {
            name: "nobody-here".to_owned(),
            confirm: true,
        },
        &ctx,
    )
    .await;
    assert!(missing.is_err(), "a name nobody holds removed somebody");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// The name is matched the way the media server matches it: without regard to case.
#[tokio::test]
async fn the_name_is_matched_however_it_is_capitalised() {
    let http = answering();
    let said = removing("cased", "ANA", false, http).await;

    assert!(
        said.is_some_and(|report| report.name == "ana"),
        "the account was not found, or was reported under the typed spelling"
    );
}

/// What a browser is handed, which is the core's promise rather than the terminal's.
///
/// A removal that serialised under the wrong name would still print correctly at a
/// prompt, so the envelope is asserted here and not left to the surface that renders it.
#[tokio::test]
async fn a_removal_serialises_under_its_own_name() {
    let env = recorded_admin("json");
    let ctx = context(&env, answering());

    let json = dispatch(
        Command::Remove {
            name: "ana".to_owned(),
            confirm: false,
        },
        &ctx,
    )
    .await
    .ok()
    .and_then(|outcome| outcome.envelope().to_json())
    .unwrap_or_default();

    assert!(json.contains(r#""kind":"removal""#), "{json}");
    assert!(json.contains("ana"), "{json}");
    // The figure the operator is deciding on has to survive the boundary.
    assert!(json.contains(r#""requests":2"#), "{json}");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// The shipped stack with one service's block taken out.
///
/// Leaked because `Source::External` holds a `&'static Path`, and kept per-service so
/// two tests wanting different omissions do not share one directory.
fn stack_without(service: &str, tag: &str) -> Source {
    let from = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    ));
    let to = std::env::temp_dir().join(format!("lemonfiber-without-{tag}-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&to);
    let read = std::fs::read_to_string(from.join("stack.toml")).unwrap_or_default();
    let needle = format!("id = \"{service}\"");
    let kept: String = read
        .split("[[service]]")
        .filter(|block| !block.contains(&needle))
        .collect::<Vec<_>>()
        .join("[[service]]");
    let _ = std::fs::write(to.join("stack.toml"), kept);
    Source::External(Box::leak(to.into_boxed_path()))
}

/// A context over a stack the caller chose.
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
            ..Settings::default()
        },
        Environment::MacOs,
    )
    .with_http(http)
}

/// A stack that cannot be read is an error rather than an empty household.
///
/// Nothing is removed and nothing is claimed about who is here: a manifest that will
/// not parse says nothing about the household, and reporting nobody would be a claim.
#[tokio::test]
async fn a_stack_that_cannot_be_read_removes_nobody() {
    let env = recorded_admin("unreadable-stack");
    let ctx = context_on(
        &env,
        answering(),
        Source::External(std::path::Path::new("/lemonfiber/no/such/stack")),
    );

    let refused = dispatch(
        Command::Remove {
            name: "ana".to_owned(),
            confirm: true,
        },
        &ctx,
    )
    .await;

    assert!(refused.is_err(), "an unreadable stack removed somebody");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A stack with no media server has no household to remove anybody from.
#[tokio::test]
async fn a_stack_with_no_media_server_has_nobody_to_remove() {
    let env = recorded_admin("no-server");
    let ctx = context_on(&env, answering(), stack_without("jellyfin", "server"));

    let refused = dispatch(
        Command::Remove {
            name: "ana".to_owned(),
            confirm: true,
        },
        &ctx,
    )
    .await;

    assert!(
        refused.is_err(),
        "a stack with no media server removed somebody"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A stack with no request service costs nothing there, and asks it nothing.
#[tokio::test]
async fn a_stack_with_no_request_service_counts_nothing_and_asks_nothing() {
    let env = recorded_admin("no-asking");
    let http = answering();
    let ctx = context_on(&env, http.clone(), stack_without("seerr", "asking"));

    let said = dispatch(
        Command::Remove {
            name: "ana".to_owned(),
            confirm: true,
        },
        &ctx,
    )
    .await;

    let Some(Outcome::Removed(report)) = said.ok() else {
        unreachable!("a stack with no request service still removes the account")
    };
    assert_eq!(
        report.requests, 0,
        "requests were counted with nothing to count them from"
    );
    assert!(!report.asks_through_the_request_service);
    assert_eq!(report.revoked, Revoked::Everywhere);
    assert!(
        !http
            .requests()
            .iter()
            .any(|request| request.url.contains("/api/v1/")),
        "a stack with no request service was asked something"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A media server that will not say who it holds removes nobody.
#[tokio::test]
async fn a_media_server_that_will_not_say_who_is_here_removes_nobody() {
    let env = recorded_admin("unreadable");
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#)],
        ),
        ("/Users", vec![Answer::reply(200, "not json")]),
        ("", vec![Answer::reply(200, "[]")]),
    ]);
    let ctx = context(&env, http);

    let refused = dispatch(
        Command::Remove {
            name: "ana".to_owned(),
            confirm: true,
        },
        &ctx,
    )
    .await;

    assert!(refused.is_err(), "an unreadable household removed somebody");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A media server that refuses the removal says so, and the request service is left.
#[tokio::test]
async fn a_media_server_that_refuses_the_removal_leaves_the_other_account_alone() {
    let env = recorded_admin("refused");
    let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![signed_in.clone(), signed_in],
        ),
        ("/auth/jellyfin", vec![Answer::reply(200, "{}")]),
        (
            "/api/v1/request",
            vec![Answer::reply(
                200,
                r#"{"pageInfo":{"results":0},"results":[]}"#,
            )],
        ),
        ("/user/jellyfin/", vec![Answer::reply(200, r#"{"id":7}"#)]),
        ("/Users/9", vec![Answer::reply(500, "")]),
        ("/Users", vec![Answer::reply(200, HOUSEHOLD)]),
        ("", vec![Answer::reply(200, "[]")]),
    ]);
    let ctx = context(&env, http.clone());

    let refused = dispatch(
        Command::Remove {
            name: "ana".to_owned(),
            confirm: true,
        },
        &ctx,
    )
    .await;

    assert!(refused.is_err(), "a refused removal was reported as done");
    assert!(
        !http
            .requests()
            .iter()
            .any(|request| request.url.contains("/api/v1/user/7")
                && matches!(request.method, lemonfiber_core::ports::http::Method::Delete)),
        "the request service's account was taken although the media server's was not"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A request service that will not say what it holds is said, not guessed at.
///
/// Both ways it can fail to say — refusing the sign-in, and refusing the lookup — reach
/// the same place, and neither may read as "they have no account there".
#[tokio::test]
async fn a_request_service_that_will_not_say_what_it_holds_is_reported() {
    for (tag, sign_in, lookup) in [
        (
            "no-session",
            Answer::reply(500, ""),
            Answer::reply(200, r#"{"id":7}"#),
        ),
        (
            "no-lookup",
            Answer::reply(200, "{}"),
            Answer::reply(403, ""),
        ),
    ] {
        let env = recorded_admin(tag);
        let signed_in = Answer::reply(200, r#"{"AccessToken":"token"}"#);
        let http = Fake::by_path_in_turn(vec![
            (
                "/Users/AuthenticateByName",
                vec![signed_in.clone(), signed_in],
            ),
            ("/auth/jellyfin", vec![sign_in]),
            (
                "/api/v1/request",
                vec![Answer::reply(
                    200,
                    r#"{"pageInfo":{"results":0},"results":[]}"#,
                )],
            ),
            ("/user/jellyfin/", vec![lookup]),
            ("/Users/9", vec![Answer::reply(204, "")]),
            ("/Users", vec![Answer::reply(200, HOUSEHOLD)]),
            ("", vec![Answer::reply(200, "[]")]),
        ]);
        let ctx = context(&env, http);

        let said = dispatch(
            Command::Remove {
                name: "ana".to_owned(),
                confirm: false,
            },
            &ctx,
        )
        .await;

        let Some(Outcome::Removed(report)) = said.ok() else {
            unreachable!("a request service that will not answer does not stop the reading")
        };
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.contains("could not be asked what it holds")),
            "{tag}: {report:?}"
        );
        assert!(
            !report.asks_through_the_request_service,
            "{tag}: a service that would not say was read as holding nothing"
        );
        let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
    }
}
