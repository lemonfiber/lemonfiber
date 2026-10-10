//! Claiming the listening server's first account, and saying who holds it.

use super::publishing::listening_server_svc;
use super::*;

/// What claiming came to, against this server's answers and this settings file.
async fn claiming(http: Arc<Fake>, env: Option<std::path::PathBuf>) -> Option<Wiring> {
    let ctx = seed_ctx(None, true, Vec::new(), Some(vec![9; 32]), env).with_http(http);
    super::super::claiming::claimed(&ctx, &[listening_server_svc()]).await
}

/// Whether the server was asked to make an account.
fn asked_to_make_one(http: &Fake) -> bool {
    http.requests()
        .iter()
        .any(|asked| asked.url.contains("/init"))
}

/// A listening server with no account gets one, and its password is kept.
///
/// Its root account goes to whoever makes the first one, from anywhere on the
/// network, so it is claimed here. Nothing is signed in for: no token is published,
/// since nothing in the stack reads one.
#[tokio::test]
async fn a_listening_server_with_no_account_is_given_one() {
    let env = recorded_admin("listening-fresh");
    let http = Fake::by_path(vec![
        ("/status", Answer::reply(200, r#"{"isInit":false}"#)),
        ("/init", Answer::reply(200, "")),
    ]);

    let wiring = claiming(http.clone(), Some(env.clone())).await;

    assert_eq!(wiring.map(|wiring| wiring.state), Some(State::Wired));
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        written.contains("AUDIOBOOKSHELF_PASSWORD="),
        "the password it was made with was not kept: {written}"
    );
    assert!(asked_to_make_one(&http), "no account was made");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A password that cannot be recorded is never set, so no account exists that
/// lemonfiber holds no password for.
#[tokio::test]
async fn a_password_that_cannot_be_recorded_makes_no_account() {
    let http = Fake::by_path(vec![
        ("/status", Answer::reply(200, r#"{"isInit":false}"#)),
        ("/init", Answer::reply(200, "")),
    ]);

    let wiring = claiming(http.clone(), None).await;

    assert!(
        wiring.as_ref().is_some_and(|wiring| matches!(
            &wiring.state,
            State::Failed { detail } if detail.contains("could not be recorded")
        )),
        "{wiring:?}"
    );
    assert!(
        !asked_to_make_one(&http),
        "an account was made with an unrecorded password"
    );
}

/// An account that was not made takes its recorded password back off, so a later
/// claim by somebody else is not read as lemonfiber's own.
#[tokio::test]
async fn an_account_that_was_not_made_leaves_no_password_recorded() {
    let env = recorded_admin("listening-refused");
    let http = Fake::by_path(vec![
        ("/status", Answer::reply(200, r#"{"isInit":false}"#)),
        ("/init", Answer::reply(500, "")),
    ]);

    let wiring = claiming(http, Some(env.clone())).await;

    assert!(
        wiring.is_some_and(|wiring| wiring.state != State::Wired),
        "an account the server refused was reported as made"
    );
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        !written.contains("AUDIOBOOKSHELF_PASSWORD"),
        "a password was left recorded for an account that was never made: {written}"
    );
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// A listening server somebody else already claimed is reported as taken, and left
/// as it is.
///
/// Whoever made that account holds the server, so saying nothing would leave an
/// operator unaware that somebody on their network took it. Making a second is refused
/// by the server, and recording a password for an account this did not make would be
/// recording one that signs in to nothing.
#[tokio::test]
async fn a_listening_server_somebody_else_claimed_is_reported_as_taken() {
    let env = recorded_admin("set-up-elsewhere");
    let http = Fake::by_path(vec![("/status", Answer::reply(200, r#"{"isInit":true}"#))]);

    let wiring = claiming(http.clone(), Some(env.clone())).await;

    assert!(
        wiring.as_ref().is_some_and(|wiring| matches!(
            &wiring.state,
            State::Failed { detail } if detail.contains("account lemonfiber did not make")
        )),
        "{wiring:?}"
    );
    let written = std::fs::read_to_string(&env).unwrap_or_default();
    assert!(
        !written.contains("AUDIOBOOKSHELF_PASSWORD"),
        "a password was recorded for an account somebody else made: {written}"
    );
    assert!(!asked_to_make_one(&http), "a second account was asked for");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// An account lemonfiber made on an earlier run is its own, and says so.
#[tokio::test]
async fn an_account_lemonfiber_made_earlier_is_already_wired() {
    let env = recorded_admin("listening-ours");
    let _ = store::set(
        &env,
        crate::config::LISTENING_SERVER_PASSWORD_KEY,
        "minted-earlier",
    );
    let http = Fake::by_path(vec![("/status", Answer::reply(200, r#"{"isInit":true}"#))]);

    let wiring = claiming(http.clone(), Some(env.clone())).await;

    assert_eq!(wiring.map(|wiring| wiring.state), Some(State::AlreadyWired));
    assert!(!asked_to_make_one(&http), "a second account was asked for");
    let _ = std::fs::remove_dir_all(env.parent().unwrap_or(std::path::Path::new("/")));
}

/// Nothing to claim where the stack has no listening server, and nothing claimed by a
/// run that only says what it would do.
#[tokio::test]
async fn nothing_is_claimed_without_a_listening_server_or_on_a_rehearsal() {
    let http = Fake::always(Answer::reply(200, r#"{"isInit":false}"#));
    let ctx = seed_ctx(None, true, Vec::new(), Some(vec![9; 32]), None).with_http(http.clone());
    assert!(super::super::claiming::claimed(&ctx, &[]).await.is_none());
    let rehearsing = ctx.rehearsing();
    assert!(
        super::super::claiming::claimed(&rehearsing, &[listening_server_svc()])
            .await
            .is_none()
    );
    assert!(http.requests().is_empty(), "a server was asked something");
}

/// A listening server that does not answer is left for a later run.
#[tokio::test]
async fn a_listening_server_that_does_not_answer_is_left_for_a_later_run() {
    let wiring = claiming(Fake::always(Answer::Silent), None).await;
    assert!(wiring.is_some_and(|wiring| is_skipped(&wiring)));
}
