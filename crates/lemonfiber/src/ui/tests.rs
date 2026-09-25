use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderValue, StatusCode};
use axum::Router;
use lemonfiber_api::admission::Admitting;
use lemonfiber_api::events::live::Live;
use lemonfiber_api::events::Streaming;
use lemonfiber_api::guard::Binding;
use lemonfiber_api::guard::Token;
use lemonfiber_api::jobs::Jobs;
use lemonfiber_api::router::Serving;
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::Settings;
use lemonfiber_core::platform::HostOs;
use lemonfiber_core::ports::process::Runner;
use lemonfiber_fixtures::ports::{Chance, Idle};

use super::fixtures::{bound, exited, missing};
use super::reach::{held, Offered, Reach};
use super::said::opening;
use super::LOOK;
use super::{address, app, run, serving, surface, tokenless, Asked, Browser, NOT_A_PORT};
use clap::Parser as _;
use lemonfiber_core::admission as credential;
use lemonfiber_fixtures::support::a_password;
use std::path::PathBuf;

use crate::prompt::fixtures::Script;

#[test]
fn a_named_directory_is_the_app_instead_of_the_embedded_one() {
    let named = app(None, Some("/srv/app".into()));
    assert!(
        matches!(named, Some(lemonfiber_core::frontend::Source::External(path))
            if path == std::path::Path::new("/srv/app"))
    );
}

#[test]
fn a_build_with_no_app_and_no_directory_has_none() {
    assert!(app(None, None).is_none());
}

#[test]
fn a_token_this_machine_will_not_supply_is_reported_rather_than_invented() {
    let problem = tokenless();
    assert!(!problem.remedies.is_empty(), "somewhere to go");
    assert!(problem.summary.contains("token"), "{}", problem.summary);
}

/// What a command line comes to, or nothing where it is not a request to serve.
///
/// Through the parser rather than by building the flags here: what these pin is
/// the command line's own defaults, and defaults written down a second time
/// agree today and drift on the day one of them changes.
fn asked_for(said: &[&str]) -> Option<Asked> {
    let parsed = lemonfiber::cli::Cli::try_parse_from(said).ok()?;
    match parsed.command {
        Some(lemonfiber::cli::Request::Ui(raw)) => Some(Asked::from(raw)),
        _ => None,
    }
}

/// The screen and the command line start from the same place, or the two presses
/// that key has always taken would quietly mean something else.
#[test]
fn asking_for_nothing_is_what_the_screen_starts_from() {
    assert_eq!(asked_for(&["lemonfiber", "ui"]), Some(Asked::unsaid()));
}

/// Each flag reaches the choice it names, since a flag that reaches none is a
/// flag that silently does nothing.
#[test]
fn each_flag_reaches_the_choice_it_names() {
    assert_eq!(
        asked_for(&[
            "lemonfiber",
            "ui",
            "--port",
            "7171",
            "--no-browser",
            "--assets",
            "/srv/app",
            "--set-password",
            "--lan",
        ]),
        Some(Asked {
            port: Some(7171),
            browser: false,
            assets: Some(PathBuf::from("/srv/app")),
            password: true,
            reach: Reach::Network,
        })
    );
}

/// A port the command line will not read never reaches this surface at all,
/// which is the shell's half of the check the screen makes for itself.
#[test]
fn a_port_the_command_line_will_not_read_never_reaches_this_surface() {
    assert_eq!(asked_for(&["lemonfiber", "ui", "--port", "seventy"]), None);
    assert_eq!(asked_for(&["lemonfiber", "version"]), None);
}

/// A word is a port or it is refused, rather than rounded to something.
#[test]
fn a_word_typed_where_a_port_goes_is_one_or_it_is_refused() {
    let asked = Asked::unsaid();

    for (said, port) in [("7171", 7171), (" 7171 ", 7171), ("0", 0)] {
        assert_eq!(
            asked.on_port(said).map(|asked| asked.port),
            Ok(Some(port)),
            "{said:?}"
        );
    }
    for said in ["seventy", "-1", "65536", "71.71", "7171x"] {
        assert_eq!(
            asked.on_port(said).map(|asked| asked.port),
            Err(NOT_A_PORT),
            "{said:?}"
        );
    }
}

/// Naming no port is a request rather than an omission: it asks for whichever
/// one is free, which is what naming no flag asks for.
#[test]
fn naming_no_port_at_a_screen_asks_for_whichever_one_is_free() {
    for said in ["", "   "] {
        assert_eq!(
            Asked::unsaid().on_port(said).map(|asked| asked.port),
            Ok(None),
            "{said:?}"
        );
    }
}

/// Naming no directory is the interface this program was built with, which is
/// what naming no flag asks for.
#[test]
fn naming_no_directory_is_the_interface_built_into_this_program() {
    for said in ["", "  "] {
        assert!(
            Asked::unsaid().serving_from(said).assets.is_none(),
            "{said:?}"
        );
    }
}

/// Filling one choice leaves the other four where they were, or a screen setting
/// a port would be taking a browser away with it.
#[test]
fn filling_one_choice_leaves_the_other_four_alone() {
    let asked = Asked::unsaid()
        .serving_from("/srv/app")
        .turned()
        .asking()
        .reaching();

    assert_eq!(
        asked.on_port("7171"),
        Ok(Asked {
            port: Some(7171),
            browser: false,
            assets: Some(PathBuf::from("/srv/app")),
            password: true,
            reach: Reach::Network,
        })
    );
    assert_eq!(
        asked.turned().asking().reaching(),
        Asked::unsaid().serving_from("/srv/app")
    );
}

#[test]
fn asking_for_nothing_in_particular_asks_for_nothing_in_particular() {
    let asked = Asked::default();
    assert_eq!(asked.port, None);
    assert!(
        !asked.browser,
        "a browser is opened only where it is wanted"
    );
    assert!(asked.assets.is_none());
}

// ── Starting the whole of it, and stopping it again ───────────────────────

/// A context over the stack this binary ships, with the randomness a test
/// chose and the runner it wants every program answered by.
fn running(runner: Arc<dyn Runner>, bytes: Option<Vec<u8>>, settings: Settings) -> Ctx {
    lemonfiber_testing::a_live_context()
        .runner(runner)
        .over(lemonfiber_core::stack::Source::Embedded(
            &lemonfiber::carried::STACK,
        ))
        .settings(settings)
        .build()
        .with_random(Arc::new(Chance::exactly(bytes)))
}

/// The same, over a runner that spawns nothing.
fn ctx(bytes: Option<Vec<u8>>) -> Ctx {
    running(Arc::new(Idle), bytes, Settings::default())
}

/// The same, keeping a password wherever a test says.
fn keeping(admission: Option<PathBuf>) -> Ctx {
    running(
        Arc::new(Idle),
        Some(enough()),
        Settings {
            admission,
            ..Settings::default()
        },
    )
}

/// A directory of this test's own, emptied first so a rerun starts fresh.
fn a_directory(named: &str) -> PathBuf {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("ui-{named}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Bytes enough to mint a token from.
fn enough() -> Vec<u8> {
    vec![0x0a; 32]
}

/// The token those bytes are written as, which a test sends back.
fn written() -> String {
    "0a".repeat(32)
}

/// Start the surface, stop it at once, and say what it exited with.
async fn started(ctx: Ctx, asked: Asked) -> String {
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(serving(
        ctx,
        asked,
        None,
        Box::pin(async move {
            let _ = stopped.await;
        }),
        LOOK,
    ));
    // Let the loop reach the socket before it is asked to leave it, so this
    // proves a surface that started rather than one that never did.
    tokio::task::yield_now().await;
    let _ = stop.send(());
    running.await.map(crate::exit::shown).unwrap_or_default()
}

#[tokio::test]
async fn a_surface_that_started_stops_cleanly_when_it_is_asked_to() {
    assert_eq!(
        started(ctx(Some(enough())), Asked::default()).await,
        crate::exit::shown(ExitCode::SUCCESS)
    );
}

#[tokio::test]
async fn no_way_of_failing_to_open_a_browser_fails_the_command() {
    // The two ways it goes wrong: the program is not there at all, and the program
    // is there and would not do it. Each is checked to be the failing shape and
    // then driven through the whole of a run — a mapping that quietly called one of
    // them success would otherwise leave this passing on a run that never met a
    // browser which would not open.
    let asked = Asked {
        browser: true,
        ..Asked::default()
    };
    for runner in [missing(), exited(1)] {
        assert_eq!(
            opening(&runner, HostOs::Linux, &address(bound())).await,
            Browser::Unopened
        );
        assert_eq!(
            started(
                running(Arc::new(runner), Some(enough()), Settings::default()),
                asked.clone()
            )
            .await,
            crate::exit::shown(ExitCode::SUCCESS)
        );
    }
}

#[tokio::test]
async fn a_port_already_held_stops_the_run_rather_than_serving_on_another() {
    // Held for the length of this test, and asked for again by name. If the
    // first take had failed the port would be absent and the run would
    // succeed, which this would then report — a wrong answer either way is
    // an assertion that fails, never a branch nothing runs.
    let taken = held(Offered::Machine, None).await.ok();
    let asked = Asked {
        port: taken
            .as_ref()
            .and_then(|taken| taken.first())
            .map(|(_, bound)| bound.port()),
        ..Asked::default()
    };
    let code = serving(
        ctx(Some(enough())),
        asked,
        None,
        Box::pin(std::future::ready(())),
        LOOK,
    )
    .await;
    assert_ne!(
        crate::exit::shown(code),
        crate::exit::shown(ExitCode::SUCCESS)
    );
    drop(taken);
}

#[tokio::test]
async fn a_machine_that_will_not_supply_randomness_serves_nothing() {
    // A surface whose token could not be minted would be one every request
    // reached, so there is nothing here to fall back to.
    let code = serving(
        ctx(None),
        Asked::default(),
        None,
        Box::pin(std::future::ready(())),
        LOOK,
    )
    .await;
    assert_ne!(
        crate::exit::shown(code),
        crate::exit::shown(ExitCode::SUCCESS)
    );
}

// ── What a request to the running surface meets ───────────────────────────

/// The surface a run builds, or nothing where the machine gave no token.
///
/// Both answers are asked for below, so neither is a line nothing runs —
/// this module is under the same coverage gate as the code it tests.
fn as_served(random: &Chance) -> Option<Router> {
    let token = Arc::new(Token::mint(random)?);
    let live = Arc::new(Live::opening(
        lemonfiber_fixtures::ports::Stopped::at(0).as_ref(),
    ));
    let admitting = Arc::new(Admitting::default());
    let serving = Serving {
        ctx: Arc::new(ctx(Some(enough()))),
        token: Arc::clone(&token),
        bound: Binding::here(bound().port()),
        jobs: Jobs::default(),
        admitting: Arc::clone(&admitting),
        live: Arc::clone(&live),
    };
    let streaming = Arc::new(Streaming {
        token,
        bound: Binding::here(bound().port()),
        admitting,
        live,
        clock: Arc::clone(&serving.ctx.seams.clock),
    });
    Some(surface(serving, streaming, None))
}

/// A request to the running surface, carrying what the test chose.
///
/// Built rather than assembled through a builder: a builder hands back a
/// result whose error arm nothing here can reach.
fn asking(action: Option<&str>, token: Option<&str>) -> Request {
    let mut request = Request::new(Body::from("{}"));
    let asked = action.map_or_else(|| "/".to_owned(), |action| format!("/api/actions/{action}"));
    if action.is_some() {
        *request.method_mut() = axum::http::Method::POST;
    }
    *request.uri_mut() = asked.parse().unwrap_or_default();
    let headers = request.headers_mut();
    headers.insert("host", HeaderValue::from_static("127.0.0.1:8471"));
    headers.insert("content-type", HeaderValue::from_static("application/json"));
    if let Some(token) = token {
        headers.insert(
            "x-lemonfiber-token",
            HeaderValue::from_str(token).unwrap_or(HeaderValue::from_static("")),
        );
    }
    request
}

/// What a request to that surface is answered with.
async fn met(surface: Option<Router>, action: Option<&str>, token: Option<&str>) -> Option<u16> {
    answering(surface, asking(action, token)).await
}

/// The same, for a request a caller built itself.
///
/// The one place a surface that was never built is answered for, so a test
/// asking about a path rather than an action does not carry a second arm for
/// the case only one test reaches.
async fn answering(surface: Option<Router>, request: Request) -> Option<u16> {
    match surface {
        Some(surface) => tower::ServiceExt::oneshot(surface, request)
            .await
            .ok()
            .map(|response| response.status().as_u16()),
        None => None,
    }
}

/// The surface as a run with a working machine builds it.
fn working() -> Option<Router> {
    as_served(&Chance::exactly(Some(enough())))
}

#[tokio::test]
async fn an_endpoint_reached_without_the_token_is_refused() {
    assert_eq!(
        met(working(), Some("up"), None).await,
        Some(StatusCode::FORBIDDEN.as_u16())
    );
}

#[tokio::test]
async fn an_endpoint_reached_with_the_token_is_admitted_and_then_answered() {
    // Admitted, and then refused on its own terms: there is no such action,
    // which is a different answer from not being let in at all.
    assert_eq!(
        met(working(), Some("reticulate"), Some(&written())).await,
        Some(StatusCode::NOT_FOUND.as_u16())
    );
}

#[tokio::test]
async fn the_page_itself_is_reached_without_a_token() {
    // A browser opening a page sends no header of ours, and it is the page
    // that goes on to ask for one. This build carries no app, so the page
    // says so rather than being refused for want of a token.
    assert_eq!(
        met(working(), None, None).await,
        Some(StatusCode::NOT_FOUND.as_u16())
    );
}

#[tokio::test]
async fn there_is_no_surface_at_all_without_a_token_to_guard_it() {
    // The other answer, and the reason the run refuses rather than serving:
    // a surface whose token could not be minted is one every request reaches.
    let unguarded = as_served(&Chance::exactly(None));
    assert_eq!(met(unguarded, Some("up"), None).await, None);
}

/// A path with no route under it is answered by the app, not by the guard.
///
/// Asked of the whole surface rather than of `router::routes`, which is the
/// difference that matters: the endpoints are merged *under* the app's
/// fallback, axum keeps one fallback per tree, and so the guarded one is not
/// the one that answers. Every test beside this one asks for a path that has a
/// route, where the guard does wrap — which is how the router's own account of
/// itself stayed wrong through the change that made it wrong.
///
/// So an unauthenticated caller can tell a path that exists from one that does
/// not. That is written down where it is true now; this holds the surface to
/// it, and will fail if the composition changes in either direction.
#[tokio::test]
async fn a_path_no_route_declares_is_an_absence_rather_than_a_refusal() {
    let mut request = Request::new(Body::empty());
    *request.uri_mut() = "/api/nothing-declares-this".parse().unwrap_or_default();
    request
        .headers_mut()
        .insert("host", HeaderValue::from_static("127.0.0.1:8471"));

    assert_eq!(
        answering(working(), request).await,
        Some(StatusCode::NOT_FOUND.as_u16()),
        "an unmatched path under `/api/` is an absence"
    );
    // And the same run refuses a path that does have a route, so the two are
    // told apart by a caller carrying no token at all.
    assert_eq!(
        met(working(), Some("up"), None).await,
        Some(StatusCode::FORBIDDEN.as_u16()),
        "while a path that has one is refused"
    );
}

// ── The password this surface asks for ────────────────────────────────────

/// A run asked for a password sets one, says so, and goes on to serve.
#[tokio::test]
async fn a_run_asked_for_a_password_sets_one_and_then_serves() {
    let dir = a_directory("kept");
    let path = dir.join("admission.json");
    let chosen = a_password();
    let answers = Script::of(&[&chosen, &chosen]);

    let code = run(
        keeping(Some(path.clone())),
        Asked {
            password: true,
            ..Asked::default()
        },
        &answers,
        None,
        Box::pin(std::future::ready(())),
    )
    .await;

    assert_eq!(
        crate::exit::shown(code),
        crate::exit::shown(ExitCode::SUCCESS)
    );
    assert!(credential::at(&path).is_some_and(|held| held.verifies(&chosen)));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every way of not getting a password serves nothing, because a run asked for
/// one and given none has not been given what it asked for — and serving anyway
/// would put the surface up under exactly the arrangement the operator was
/// changing.
#[tokio::test]
async fn a_password_that_could_not_be_set_stops_the_run_rather_than_serving() {
    let dir = a_directory("refused");
    let path = dir.join("admission.json");
    let chosen = a_password();
    let short: String = chosen.chars().take(3).collect();
    let asked = Asked {
        password: true,
        ..Asked::default()
    };

    // Nowhere to keep one; the two answers differed; the password is too short;
    // and the file cannot be written because a directory is in its place.
    assert!(std::fs::create_dir_all(dir.join("taken.json")).is_ok());
    let ways: Vec<(Option<PathBuf>, Vec<String>)> = vec![
        (None, vec![chosen.clone(), chosen.clone()]),
        (
            Some(path.clone()),
            vec![chosen.clone(), chosen.to_uppercase()],
        ),
        (Some(path.clone()), vec![short.clone(), short]),
        (
            Some(dir.join("taken.json")),
            vec![chosen.clone(), chosen.clone()],
        ),
    ];
    for (kept, said) in ways {
        let lines: Vec<&str> = said.iter().map(String::as_str).collect();
        let answers = Script::of(&lines);
        let code = run(
            keeping(kept.clone()),
            asked.clone(),
            &answers,
            None,
            Box::pin(std::future::ready(())),
        )
        .await;
        assert_ne!(
            crate::exit::shown(code),
            crate::exit::shown(ExitCode::SUCCESS),
            "{kept:?}"
        );
    }
    assert_eq!(credential::at(&path), None);
    let _ = std::fs::remove_dir_all(&dir);
}

// ── How far it is offered, and what has to be true first ──────────────────

/// A password kept where a test can take it away again.
fn a_password_at(path: &std::path::Path) {
    let held = lemonfiber_core::admission::Credential::set(&a_password(), &Chance::cycling()).ok();
    assert!(held
        .as_ref()
        .is_some_and(|held| credential::keep(path, held).is_ok()));
}

/// One request over a real connection, and the status it was answered with.
///
/// Written by hand rather than through a client, because what is being proven is
/// which requests this surface answers and a client would be a second opinion
/// about what was sent.
async fn over_tcp(port: u16, host: &str, token: &str) -> Option<u16> {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .ok()?;
    let request = format!(
        "GET /api/explain?word=indexer HTTP/1.1\r\nHost: {host}\r\n\
         X-Lemonfiber-Token: {token}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await.ok()?;
    let mut said = Vec::new();
    stream.read_to_end(&mut said).await.ok()?;
    String::from_utf8_lossy(&said)
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

/// Asking for the network without a password is refused, not warned about and not
/// quietly served on this machine instead.
#[tokio::test]
async fn the_network_without_a_password_is_refused_rather_than_served_narrower() {
    let dir = a_directory("unpassworded");
    let code = serving(
        keeping(Some(dir.join("admission.json"))),
        Asked {
            reach: Reach::Network,
            ..Asked::default()
        },
        None,
        Box::pin(std::future::ready(())),
        LOOK,
    )
    .await;
    assert_ne!(
        crate::exit::shown(code),
        crate::exit::shown(ExitCode::SUCCESS)
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// And the password taken away while it is on the network gives the network up.
///
/// Proven over a real connection rather than by reading the code back: what
/// changes is which requests are answered, and the request that tells the two
/// apart is one naming an address this machine is not — accepted while it is
/// offered to a network, refused the moment it is not.
#[tokio::test]
async fn a_password_taken_away_gives_up_the_network_and_keeps_this_machine() {
    let dir = a_directory("reverted");
    let path = dir.join("admission.json");
    a_password_at(&path);
    let free = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.ok();
    let port = free
        .as_ref()
        .and_then(|held| held.local_addr().ok())
        .map_or(0, |bound| bound.port());
    assert_ne!(port, 0, "a free port can be taken on this machine");
    drop(free);

    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let running = tokio::spawn(serving(
        keeping(Some(path.clone())),
        Asked {
            port: Some(port),
            reach: Reach::Network,
            ..Asked::default()
        },
        None,
        Box::pin(async move {
            let _ = stopped.await;
        }),
        Duration::from_millis(10),
    ));
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Offered to a network, a request naming an address this machine answers on
    // is answered — which is the whole of what being offered to a network means.
    let elsewhere = format!("203.0.113.7:{port}");
    let admitted = over_tcp(port, &elsewhere, &written()).await;
    let _ = std::fs::remove_file(&path);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let refused = over_tcp(port, &elsewhere, &written()).await;
    let here = over_tcp(port, &format!("127.0.0.1:{port}"), &written()).await;

    let _ = stop.send(());
    let ended = running.await.map(crate::exit::shown).unwrap_or_default();

    assert_eq!(admitted, Some(200), "a network binding answers an address");
    assert_eq!(refused, Some(403), "and stops the moment the password goes");
    assert_eq!(here, Some(200), "while this machine still reaches it");
    assert_eq!(ended, crate::exit::shown(ExitCode::SUCCESS));
    let _ = std::fs::remove_dir_all(&dir);
}
