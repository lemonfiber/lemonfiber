//! Everything that leaves this machine, asked for through the dispatcher.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the forms listing
//! is: the app layer is compiled twice, and a path exercised only in-crate has its
//! coverage counted from the copy that never ran.
//!
//! Nothing is faked and nothing is reached. This is a read over the settings this
//! context holds and the stack the manifest declares — no engine, no network, no files
//! of its own — and the real adapters go in so that being untouched is part of the
//! claim. A command that enumerates outbound requests and made one to answer would be
//! a poor joke.

mod common;

use std::path::Path;
use std::sync::Arc;

use common::stack::project;

use lemonfiber_core::adapters::{Daemon, Disk, Local, System};
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::{
    Reaching, Settings, OFFLINE_KEY, REACH_GUIDES_KEY, REACH_INDEXER_KEY, REACH_REGISTRY_KEY,
    REACH_USENET_KEY, SWITCHES,
};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::stack::Source;

fn ctx(stack: Source, settings: Settings) -> Ctx {
    Ctx::new(
        Arc::new(Local),
        Arc::new(Daemon::local()),
        Arc::new(System),
        Arc::new(Disk),
        stack,
        settings,
        Environment::MacOs,
    )
}

async fn listed(settings: Settings) -> lemonfiber_core::outbound::Leaving {
    let answered = dispatch(
        Command::Outbound,
        &ctx(Source::External(project()), settings),
    )
    .await;
    match answered {
        Ok(Outcome::Outbound(report)) => report,
        other => unreachable!("the enumeration answers with itself: {other:?}"),
    }
}

/// The whole of what this product asks of the world, with the setting that stops each
/// one and what stopping it costs — which is the difference between a promise and a
/// property.
#[tokio::test]
async fn every_request_this_product_makes_is_listed_with_a_way_to_stop_it() {
    let report = listed(Settings::default()).await;

    assert_eq!(report.ours.len(), 5);
    let switches: Vec<&str> = report
        .ours
        .iter()
        .map(|entry| entry.switch.as_str())
        .collect();
    for key in [
        REACH_REGISTRY_KEY,
        REACH_GUIDES_KEY,
        REACH_INDEXER_KEY,
        REACH_USENET_KEY,
    ] {
        assert!(switches.contains(&key), "{key} switches nothing off");
    }
    for entry in &report.ours {
        assert!(
            entry.cost.split_whitespace().count() >= 5,
            "{} does not say what refusing it costs",
            entry.reach.as_str()
        );
    }
}

/// The registries are read from the images the stack declares rather than written
/// down, so a stack that changes where its images come from changes this answer.
#[tokio::test]
async fn where_the_images_come_from_is_read_from_the_stack() {
    let report = listed(Settings::default()).await;

    let Some(registry) = report
        .ours
        .iter()
        .find(|entry| entry.reach.as_str() == "registry")
    else {
        unreachable!("the registry is one of the five")
    };
    assert!(
        registry.destination.contains(&"lscr.io".to_owned()),
        "{:?}",
        registry.destination
    );
    assert!(
        registry.destination.contains(&"docker.io".to_owned()),
        "{:?}",
        registry.destination
    );
}

/// Each switch turns off its own request and leaves the others alone, read back
/// through the enumeration an operator would read it back through.
#[tokio::test]
async fn each_switch_stops_one_request_and_only_that_one() {
    for switch in SWITCHES {
        let report = listed(Settings {
            reaching: Reaching::without(switch),
            ..Settings::default()
        })
        .await;
        let off: Vec<&str> = report
            .ours
            .iter()
            .filter(|entry| !entry.allowed)
            .map(|entry| entry.switch.as_str())
            .collect();
        assert_eq!(off, vec![*switch], "{switch} did not stop only its own");
    }
}

/// The blanket switch reaches the one setting that names a thing as well as answering
/// yes or no. Read through the settings reader a run reads it with, because that is
/// the path a machine actually takes to `offline` — a test that only built the
/// settings by hand would be asserting about a value nothing produced.
#[test]
fn the_blanket_switch_leaves_the_leak_check_no_source_to_ask() {
    let file = lemonfiber_core::config::env::EnvFile::parse(&format!(
        "{OFFLINE_KEY}=on\nLEMONFIBER_IP_ECHO=https://ip.example\n"
    ));

    assert!(lemonfiber_core::config::ip_echo_from_env(&file).is_empty());
    assert!(lemonfiber_core::config::offline(&file));
}

/// The state the specification calls `offline`, read back as what it comes to: not one
/// request left on.
#[tokio::test]
async fn a_machine_that_is_offline_makes_no_request_of_its_own_at_all() {
    let report = listed(Settings {
        ip_echo: Vec::new(),
        reaching: Reaching::none(),
        ..Settings::default()
    })
    .await;

    let still_on: Vec<&str> = report
        .ours
        .iter()
        .filter(|entry| entry.allowed)
        .map(|entry| entry.reach.as_str())
        .collect();
    assert!(still_on.is_empty(), "these are still on: {still_on:?}");
    assert!(!OFFLINE_KEY.is_empty());
}

/// The stack's own requests are answered as the stack's, and every service it runs is
/// accounted for rather than only the ones that reach somewhere.
#[tokio::test]
async fn what_the_services_reach_is_attributed_to_them_and_covers_all_of_them() {
    let report = listed(Settings::default()).await;

    assert!(report.theirs.len() > 10, "{:?}", report.theirs.len());
    let Some(prowlarr) = report
        .theirs
        .iter()
        .find(|entry| entry.service == "prowlarr")
    else {
        unreachable!("the stack runs an indexer manager")
    };
    assert!(prowlarr.destination.contains("indexers"), "{prowlarr:?}");
    let quiet: Vec<&str> = report
        .theirs
        .iter()
        .filter(|entry| entry.destination.is_empty())
        .map(|entry| entry.service.as_str())
        .collect();
    assert!(
        !quiet.is_empty(),
        "every service in this stack reaches somewhere, which is not what the manifest says"
    );
}

/// A stack that will not read is a refusal, not an answer claiming its services reach
/// nothing. Half an enumeration is worse than none: it reads as the whole of it.
#[tokio::test]
async fn a_stack_that_cannot_be_read_is_refused_rather_than_answered_as_reaching_nothing() {
    let answered = dispatch(
        Command::Outbound,
        &ctx(
            Source::External(Path::new("/no/such/stack")),
            Settings::default(),
        ),
    )
    .await;

    assert!(answered.is_err());
}

/// Asking to fetch when fetching is switched off is refused rather than run, and the
/// refusal names the setting — a run that reported a fetch nothing fetched would be the
/// worse of the two answers.
#[tokio::test]
async fn a_fetch_is_refused_by_name_when_fetching_is_switched_off() {
    let refused = dispatch(
        Command::Pull {
            forms: vec!["library".to_owned()],
        },
        &ctx(
            Source::External(project()),
            Settings {
                reaching: Reaching::without(REACH_REGISTRY_KEY),
                ..Settings::default()
            },
        ),
    )
    .await;

    let Err(problem) = refused else {
        unreachable!("a fetch this machine refuses does not run")
    };
    assert!(problem.meaning.contains(REACH_REGISTRY_KEY), "{problem:?}");
}

/// Nothing unexpected leaves this machine during a whole diagnosis.
///
/// The other guards read source text; this watches. A full non-disruptive run is the
/// widest thing this product does on its own — nine checks, several of them reaching
/// services — and every request it makes goes through the one transport, so the
/// transport is where the claim can be observed rather than argued.
///
/// What counts as expected is not a second list written here. It is the enumeration
/// itself: a request is fine if it stays on this machine, and otherwise its host must
/// be one the operator would have been told about. An address written into the code
/// and left off the list fails here even though every source sweep passed.
#[tokio::test]
async fn a_whole_diagnosis_reaches_nowhere_the_operator_was_not_told_about() {
    let http = lemonfiber_fixtures::http::Fake::silent();
    let ctx = Ctx::new(
        Arc::new(lemonfiber_fixtures::ports::Idle),
        Arc::new(lemonfiber_fixtures::support::Reporting::holding(
            &["jellyfin", "prowlarr", "sabnzbd", "qbittorrent"],
            lemonfiber_core::ports::docker::Lifecycle::Running,
            lemonfiber_core::ports::docker::Health::Healthy,
        )),
        lemonfiber_fixtures::ports::Stopped::at(1_786_000_000),
        lemonfiber_fixtures::files::Files::ending(Vec::new()),
        Source::External(project()),
        Settings::default(),
        Environment::MacOs,
    )
    .with_http(http.clone());

    let ran =
        lemonfiber_core::app::diagnose(&ctx, &lemonfiber_core::doctor::Narrowing::Suite, false)
            .await;
    assert!(ran.is_ok(), "the diagnosis ran: {ran:?}");

    let asked = http.requests();
    assert!(
        !asked.is_empty(),
        "the diagnosis asked nothing at all, so this is watching a run that did not happen"
    );
    let told_about: Vec<String> = listed(Settings::default())
        .await
        .ours
        .into_iter()
        .flat_map(|entry| entry.destination)
        .collect();
    let left: Vec<&String> = asked
        .iter()
        .map(|request| &request.url)
        .filter(|url| !stays_here(url))
        .collect();
    assert!(
        !left.is_empty(),
        "nothing in this run left the machine at all, so what is watched below is only \
         loopback traffic and the claim is about nothing"
    );
    let unexpected: Vec<String> = asked
        .iter()
        .map(|request| request.url.clone())
        .filter(|url| !stays_here(url))
        .filter(|url| !told_about.iter().any(|known| url.starts_with(known)))
        .collect();
    assert!(
        unexpected.is_empty(),
        "these left this machine and the operator is told about none of them: {unexpected:?}"
    );
}

/// Whether a URL names somewhere only this machine can reach.
///
/// The stack's own services are reached by container name on Compose's network or by
/// a loopback port, and neither is a request that leaves.
fn stays_here(url: &str) -> bool {
    let host = url
        .split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split('/')
        .next()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default();
    host == "localhost" || !host.contains('.') || host.starts_with("127.")
}

/// What was sent is written down where the enumeration says it would be.
///
/// The enumeration next door is a description of the program: which requests exist
/// and what each carries. This is the other half asked for — the record an
/// operator checks it against, made as the request went rather than reconstructed
/// afterwards from what somebody believes the program does.
///
/// Driven through the real transport decorator rather than by calling the writer,
/// because the claim is that a request *passing through* leaves a trace: a recorder
/// nothing is wrapped in records nothing, and would pass a test of the writer.
#[tokio::test]
async fn a_request_that_went_is_written_down_where_the_operator_can_read_it() {
    let at = std::env::temp_dir().join(format!(
        "lemonfiber-outbound-{}-{}.log",
        std::process::id(),
        line!()
    ));
    let _ = std::fs::remove_file(&at);

    let answering: Arc<dyn lemonfiber_core::ports::http::Http> =
        lemonfiber_fixtures::http::Fake::always(lemonfiber_fixtures::http::Answer::Reply(
            204,
            String::new(),
        ));
    let held = ctx(Source::External(project()), Settings::default())
        .with_http(answering)
        .recording_at(at.clone());
    let transport = Arc::clone(&held.http);

    let asked = lemonfiber_core::ports::http::Request {
        method: lemonfiber_core::ports::http::Method::Get,
        url: "https://indexer.example/api?apikey=the-indexer-key".to_owned(),
        headers: vec![("X-Api-Key".to_owned(), "the-indexer-key".to_owned())],
        body: None,
    };
    let answered = transport.send(&asked).await;

    let written = std::fs::read_to_string(&at).unwrap_or_default();
    let _ = std::fs::remove_file(&at);

    assert!(answered.is_ok(), "the request still happened");
    assert!(
        written.contains("indexer.example"),
        "where it went is written down: {written:?}"
    );
    assert!(written.contains("204"), "and what came back: {written:?}");
    assert!(
        !written.contains("the-indexer-key"),
        "and the credential is not, in the URL or the header: {written:?}"
    );
}
