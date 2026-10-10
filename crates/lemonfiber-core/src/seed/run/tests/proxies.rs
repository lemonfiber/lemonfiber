//! The address the media server trusts to name the client, held to the guarded front
//! door's.

use super::*;

/// The door's address on the network it reaches the media server on.
const DOOR: &str = "10.80.96.18";

/// A stack directory whose compose file fixes the door's address, or fixes none.
fn project(name: &str, door: bool) -> std::path::PathBuf {
    let at = lemonfiber_fixtures::scratch::Scratch::named(name).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(at.join("compose"));
    if door {
        let _ = std::fs::write(
            at.join("compose/media.yml"),
            format!(
                "services:\n  door:\n    networks:\n      door-upstream:\n        ipv4_address: {DOOR}\n"
            ),
        );
    }
    at
}

/// A context with the media server's administrator recorded where `administered` says.
fn proxies_ctx(name: &str, administered: bool, http: Arc<Fake>) -> Ctx {
    let at = lemonfiber_fixtures::scratch::Scratch::named(&format!("{name}-env")).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let env = at.join(".env");
    let _ = std::fs::write(&env, "DATA_ROOT=/srv/media\n");
    if administered {
        let _ = store::set(
            &env,
            crate::config::MEDIA_SERVER_ADMIN_PASSWORD_KEY,
            &lemonfiber_fixtures::support::a_password(),
        );
    }
    a_context()
        .settings(Settings {
            env_file: Some(env),
            ..Settings::default()
        })
        .build()
        .with_http(http)
}

/// A media server whose trusted list reads as `first`, then as `after` once written.
fn serving(first: &'static str, after: &'static str) -> Arc<Fake> {
    Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#); 4],
        ),
        (
            "/System/Configuration/network",
            vec![
                Answer::reply(200, first),
                Answer::reply(200, first),
                Answer::reply(204, ""),
                Answer::reply(200, after),
            ],
        ),
        ("/System/Restart", vec![Answer::reply(204, "")]),
    ])
}

const NOBODY: &str = r#"{"KnownProxies":[]}"#;
const THE_DOOR: &str = r#"{"KnownProxies":["10.80.96.18"]}"#;

/// The one wiring this pass reports, and whether it asked for a restart.
async fn seeded(ctx: &Ctx, http: &Fake, project: &std::path::Path) -> (Option<State>, bool) {
    let stack = vec![media_server_svc()];
    let wiring =
        super::super::proxies::seed_proxies(ctx, Some(project), served(&stack).as_ref()).await;
    let restarted = http
        .requests()
        .iter()
        .any(|asked| asked.url.ends_with("/System/Restart"));
    (wiring.map(|one| one.state), restarted)
}

/// A server trusting nobody is told to trust the door, and started again to read it.
#[tokio::test(start_paused = true)]
async fn a_server_trusting_nobody_trusts_the_door_and_starts_again() {
    let http = serving(NOBODY, THE_DOOR);
    let ctx = proxies_ctx("proxies-nobody", true, http.clone());
    let (state, restarted) = seeded(&ctx, &http, &project("proxies-nobody", true)).await;
    assert_eq!(state, Some(State::Wired));
    assert!(restarted);
}

/// A server already trusting the door is left alone and not restarted.
#[tokio::test(start_paused = true)]
async fn a_server_already_trusting_the_door_is_left_running() {
    let http = serving(THE_DOOR, THE_DOOR);
    let ctx = proxies_ctx("proxies-already", true, http.clone());
    let (state, restarted) = seeded(&ctx, &http, &project("proxies-already", true)).await;
    assert_eq!(state, Some(State::AlreadyWired));
    assert!(!restarted);
}

/// A rehearsal says what it would trust and writes nothing.
#[tokio::test(start_paused = true)]
async fn a_rehearsal_says_what_it_would_trust_and_writes_nothing() {
    let http = serving(NOBODY, THE_DOOR);
    let mut ctx = proxies_ctx("proxies-rehearsed", true, http.clone());
    ctx.dry_run = true;
    let (state, restarted) = seeded(&ctx, &http, &project("proxies-rehearsed", true)).await;
    assert!(matches!(state, Some(State::WouldWire { ours: Some(ours), .. }) if ours == DOOR));
    assert!(!restarted);

    let unset = proxies_ctx("proxies-rehearsed-unset", false, serving(NOBODY, NOBODY));
    let mut unset = unset;
    unset.dry_run = true;
    let (state, _) = seeded(&unset, &http, &project("proxies-rehearsed-unset", true)).await;
    assert!(matches!(state, Some(State::WouldWire { yours: None, .. })));
}

/// No door, or no administrator on a real run, is nothing to hold.
#[tokio::test(start_paused = true)]
async fn no_door_and_no_administrator_hold_nothing() {
    let http = serving(NOBODY, THE_DOOR);
    let ctx = proxies_ctx("proxies-no-door", true, http.clone());
    let (state, _) = seeded(&ctx, &http, &project("proxies-no-door", false)).await;
    assert_eq!(state, None);
    let ctx = proxies_ctx("proxies-no-admin", false, http.clone());
    let (state, _) = seeded(&ctx, &http, &project("proxies-no-admin", true)).await;
    assert_eq!(state, None);
}

/// A server that will not take the list is said as unreached.
#[tokio::test(start_paused = true)]
async fn a_server_that_will_not_answer_is_unreached() {
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            vec![Answer::reply(200, r#"{"AccessToken":"token"}"#); 4],
        ),
        (
            "/System/Configuration/network",
            vec![Answer::reply(500, ""); 4],
        ),
    ]);
    let ctx = proxies_ctx("proxies-unreached", true, http.clone());
    let (state, _) = seeded(&ctx, &http, &project("proxies-unreached", true)).await;
    assert!(state.is_some_and(|state| !matches!(state, State::Wired | State::AlreadyWired)));
}

/// A server that does not come back after starting again is said as unreached, after
/// every read it was given.
#[tokio::test(start_paused = true)]
async fn a_server_that_does_not_come_back_is_unreached() {
    let http = Fake::by_path_in_turn(vec![
        (
            "/Users/AuthenticateByName",
            [
                vec![Answer::reply(200, r#"{"AccessToken":"token"}"#)],
                vec![Answer::reply(503, ""); 60],
            ]
            .concat(),
        ),
        (
            "/System/Configuration/network",
            [
                vec![
                    Answer::reply(200, NOBODY),
                    Answer::reply(200, NOBODY),
                    Answer::reply(204, ""),
                    Answer::reply(200, THE_DOOR),
                ],
                vec![Answer::reply(503, ""); 60],
            ]
            .concat(),
        ),
        ("/System/Restart", vec![Answer::reply(204, "")]),
    ]);
    let ctx = proxies_ctx("proxies-gone", true, http.clone());
    let (state, restarted) = seeded(&ctx, &http, &project("proxies-gone", true)).await;
    assert!(restarted);
    assert!(state.is_some_and(|state| !matches!(state, State::Wired | State::AlreadyWired)));
}
