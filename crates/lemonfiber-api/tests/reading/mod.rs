//! What both halves of the reading tests are built from.
//!
//! A shared module rather than a copy in each file: these settle what a request
//! carries and what a context holds, and two copies of that would answer the same
//! question differently the first time one of them was updated.
//!
//! `tests/` compiles each file as its own crate and this module into both of them,
//! so anything one half uses is unused in the other — a property of how integration
//! tests are built rather than of what is written here. Both lints are suppressed
//! for that and no other reason; the alternative is a copy per half, which is the
//! thing this module exists to avoid.
#![allow(dead_code, unused_imports)]

pub(crate) use axum::body::{to_bytes, Body};
pub(crate) use axum::http::{Request, StatusCode};
pub(crate) use lemonfiber_api::events::live::Live;
pub(crate) use lemonfiber_api::events::Streaming;
pub(crate) use lemonfiber_api::guard::{Binding, Token, TOKEN_HEADER};
pub(crate) use lemonfiber_api::jobs::Jobs;
pub(crate) use lemonfiber_api::read::enveloped;
pub(crate) use lemonfiber_api::reads;
pub(crate) use lemonfiber_api::router::{routes, Serving};
pub(crate) use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome, QualityAction};
pub(crate) use lemonfiber_core::config::store::REDACTED;
pub(crate) use lemonfiber_core::config::Settings;
pub(crate) use lemonfiber_core::platform::Environment;
pub(crate) use lemonfiber_core::ports::docker::{Health, Lifecycle};
pub(crate) use lemonfiber_core::stack::Source;
pub(crate) use lemonfiber_fixtures::http::Fake;
pub(crate) use lemonfiber_fixtures::ports::{Chance, Idle, Renamed, Stopped};
pub(crate) use lemonfiber_fixtures::support::Reporting;
pub(crate) use std::path::Path;
pub(crate) use std::sync::Arc;
pub(crate) use tower::ServiceExt as _;

/// Bytes the test chose, so a token is the same one twice.
///
/// Cycled to whatever width is asked for: a source that answers short mints no
/// token at all, and every request here would then be refused for that instead of
/// answered for what it asked.
pub(crate) fn given() -> Chance {
    Chance::cycling()
}

/// The token every request here carries, read back from the run that minted it.
pub(crate) fn written() -> Option<String> {
    Token::mint(&given()).map(|token| token.as_str().to_owned())
}

/// Serving this machine and nowhere else, at the port these tests name.
pub(crate) fn bound() -> Binding {
    Binding::here(8471)
}

/// The stack this repository carries, read from disk.
pub(crate) fn stack() -> Source {
    Source::External(Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A stack source pointing nowhere, for the reads that are about a refusal.
pub(crate) fn nowhere() -> Source {
    Source::External(Path::new("/lemonfiber/no/such/stack"))
}

/// The world a command runs against: nothing is spawned and nothing is fetched,
/// so what an endpoint answers is decided by what the engine reports.
pub(crate) fn world(engine: Reporting, stack: Source) -> Ctx {
    holding(engine, stack, Settings::default())
}

/// The same world, told where the settings it reads are kept.
pub(crate) fn holding(engine: Reporting, stack: Source, settings: Settings) -> Ctx {
    Ctx::new(
        Arc::new(Idle),
        Arc::new(engine),
        Arc::new(lemonfiber_core::adapters::System),
        Arc::new(lemonfiber_core::adapters::Disk),
        stack,
        settings,
        Environment::MacOs,
    )
    .with_http(Fake::silent())
}

/// A value built rather than written, so nothing scanning this tree for a
/// committed credential finds a string that reads as one.
pub(crate) fn a_value() -> String {
    ('a'..='j').collect()
}

/// Two settings: one ordinary, and one whose name reads as a credential.
pub(crate) fn kept() -> String {
    format!("LEMONFIBER_USENET=on\nSONARR_API_KEY={}\n", a_value())
}

/// A world whose settings are these, written to a scratch file this test owns.
pub(crate) fn configured(named: &str, contents: &str) -> Ctx {
    let dir = std::env::temp_dir().join(format!("lemonfiber-read-{}-{named}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(".env");
    let _ = std::fs::write(&path, contents);
    holding(
        running(),
        stack(),
        Settings {
            env_file: Some(path),
            ..Settings::default()
        },
    )
}

/// An engine reporting one healthy service.
pub(crate) fn running() -> Reporting {
    Reporting::holding(&["jellyfin"], Lifecycle::Running, Health::Healthy)
}

/// The status and body a request to this path is answered with.
pub(crate) async fn asked(ctx: Ctx, path: &str) -> Option<(StatusCode, String)> {
    let carried = written()?;
    answered(
        ctx,
        path,
        &[("host", "127.0.0.1:8471"), (TOKEN_HEADER, &carried)],
    )
    .await
}

/// The same, for a request that says something else about itself.
pub(crate) async fn answered(
    ctx: Ctx,
    path: &str,
    said: &[(&str, &str)],
) -> Option<(StatusCode, String)> {
    let token = Arc::new(Token::mint(&given())?);
    let live = Arc::new(Live::opening(Stopped::at(0).as_ref()));
    let router = routes(
        Serving {
            ctx: Arc::new(ctx),
            token: Arc::clone(&token),
            bound: bound(),
            admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
            jobs: Jobs::default(),
            live: Arc::clone(&live),
        },
        Arc::new(Streaming {
            admitting: Arc::new(lemonfiber_api::admission::Admitting::default()),
            token,
            bound: bound(),
            live,
        }),
    );

    let mut building = Request::builder().uri(path);
    for (name, value) in said {
        building = building.header(*name, *value);
    }
    let response = router
        .oneshot(building.body(Body::empty()).ok()?)
        .await
        .ok()?;

    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.ok()?;
    Some((status, String::from_utf8(body.to_vec()).ok()?))
}

/// The envelope the command line would have rendered for this command.
pub(crate) async fn as_the_command_renders_it(ctx: &Ctx, command: Command) -> Option<String> {
    dispatch(command, ctx)
        .await
        .ok()
        .map(Outcome::envelope)
        .and_then(|envelope| envelope.to_json())
}
