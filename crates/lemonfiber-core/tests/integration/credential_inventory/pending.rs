//! A replacement a rotation left beside the credential it replaces, settled at the start
//! of the next command by asking the service which of the two it takes.

use super::{ctx, env_at, recorded, the_torrent_password};
use lemonfiber_core::app::{dispatch, Command};
use lemonfiber_core::config::{JELLYFIN_ADMIN_PASSWORD_KEY, QBITTORRENT_PASSWORD_KEY};
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};
use std::sync::Arc;

/// The name a torrent client's replacement is kept under.
const QBITTORRENT_PENDING: &str = "QBITTORRENT_PASSWORD_PENDING";

/// The replacement the earlier run left pending, built as the others are.
fn the_replacement() -> String {
    format!("{}{}", "aaaa9999", "bbbb8888cccc")
}

/// A command that asks no service anything of its own, so every request is the settling.
async fn any_command(env: &std::path::Path, http: Arc<Fake>, rehearsing: bool) {
    let mut ctx = ctx(env.to_path_buf(), Files::empty(), http);
    ctx.dry_run = rehearsing;
    let _ = dispatch(Command::Forms, &ctx).await;
}

/// A settings file holding the password in force and a replacement beside it.
fn both(name: &str) -> std::path::PathBuf {
    env_at(
        name,
        &[
            (QBITTORRENT_PASSWORD_KEY, &the_torrent_password()),
            (QBITTORRENT_PENDING, &the_replacement()),
        ],
    )
}

/// The client refused the password in force and took the replacement, so the
/// replacement is moved into place.
#[tokio::test]
async fn a_replacement_the_service_took_is_moved_into_place() {
    let env = both("pending-taken");
    let http = Fake::by_path_in_turn(vec![(
        "/auth/login",
        vec![Answer::reply(200, "Fails."), Answer::reply(200, "Ok.")],
    )]);
    any_command(&env, http, false).await;
    assert_eq!(
        recorded(&env, QBITTORRENT_PASSWORD_KEY),
        Some(the_replacement())
    );
    assert_eq!(recorded(&env, QBITTORRENT_PENDING), None);
}

/// The client still takes the password in force, so the replacement goes.
#[tokio::test]
async fn a_replacement_the_service_never_took_is_taken_away() {
    let env = both("pending-untaken");
    let http = Fake::by_path(vec![("/auth/login", Answer::reply(200, "Ok."))]);
    any_command(&env, http, false).await;
    assert_eq!(
        recorded(&env, QBITTORRENT_PASSWORD_KEY),
        Some(the_torrent_password())
    );
    assert_eq!(recorded(&env, QBITTORRENT_PENDING), None);
}

/// Where the client takes neither, will not answer, or the run only says what it would
/// do, both stay for a later run to ask again.
#[tokio::test]
async fn an_unsettled_question_leaves_both_where_they_are() {
    for (name, http, rehearsing) in [
        (
            "pending-neither",
            Fake::by_path(vec![("/auth/login", Answer::reply(200, "Fails."))]),
            false,
        ),
        ("pending-silent", Fake::by_path(Vec::new()), false),
        (
            "pending-rehearsed",
            Fake::by_path(vec![("/auth/login", Answer::reply(200, "Ok."))]),
            true,
        ),
    ] {
        let env = both(name);
        any_command(&env, http, rehearsing).await;
        assert_eq!(
            recorded(&env, QBITTORRENT_PASSWORD_KEY),
            Some(the_torrent_password()),
            "{name}"
        );
        assert_eq!(
            recorded(&env, QBITTORRENT_PENDING),
            Some(the_replacement()),
            "{name}"
        );
    }
}

/// The media server's administrator password is settled the same way, through a
/// sign-in as the administrator.
#[tokio::test]
async fn the_administrators_replacement_is_settled_through_a_sign_in() {
    let env = env_at(
        "pending-administrator",
        &[
            (JELLYFIN_ADMIN_PASSWORD_KEY, &the_torrent_password()),
            ("JELLYFIN_ADMIN_PASSWORD_PENDING", &the_replacement()),
        ],
    );
    let session = r#"{"AccessToken":"token","User":{"Id":"admin-id"}}"#;
    let http = Fake::by_path_in_turn(vec![(
        "/Users/AuthenticateByName",
        vec![Answer::reply(401, ""), Answer::reply(200, session)],
    )]);
    any_command(&env, http, false).await;
    assert_eq!(
        recorded(&env, JELLYFIN_ADMIN_PASSWORD_KEY),
        Some(the_replacement())
    );
    assert_eq!(recorded(&env, "JELLYFIN_ADMIN_PASSWORD_PENDING"), None);
}

/// Nothing pending is nothing asked: every run but the one after a rotation that
/// stopped part-way costs no request at all.
#[tokio::test]
async fn nothing_pending_asks_no_service_anything() {
    let env = env_at(
        "pending-none",
        &[(QBITTORRENT_PASSWORD_KEY, &the_torrent_password())],
    );
    let http = Fake::by_path(vec![("/auth/login", Answer::reply(200, "Ok."))]);
    any_command(&env, http.clone(), false).await;
    assert!(http.requests().is_empty(), "{:?}", http.requests());
}

/// A stack that cannot be read has no service to ask, so a replacement left pending
/// stays beside the credential in force for a later run to settle.
#[tokio::test]
async fn a_stack_that_cannot_be_read_leaves_both_for_later() {
    let env = both("pending-unreadable-stack");
    let http = Fake::by_path(vec![("/auth/login", Answer::reply(200, "Ok."))]);
    let ctx = lemonfiber_testing::a_context()
        .over(lemonfiber_core::stack::Source::External(
            std::path::Path::new("/lemonfiber/no/such/stack"),
        ))
        .settings(lemonfiber_core::config::Settings {
            env_file: Some(env.clone()),
            ..lemonfiber_core::config::Settings::default()
        })
        .build()
        .with_http(http.clone());
    let _ = dispatch(Command::Forms, &ctx).await;

    assert!(http.requests().is_empty(), "{:?}", http.requests());
    assert_eq!(
        recorded(&env, QBITTORRENT_PASSWORD_KEY),
        Some(the_torrent_password())
    );
    assert_eq!(recorded(&env, QBITTORRENT_PENDING), Some(the_replacement()));
}
