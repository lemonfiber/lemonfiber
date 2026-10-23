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

use std::path::Path;
use std::sync::Arc;

use common::stack::project;

use crate::common;
use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::{
    Reaching, Settings, OFFLINE_KEY, REACH_CATALOGUE_KEY, REACH_GUIDES_KEY, REACH_HOUSEHOLD_KEY,
    REACH_INDEXER_KEY, REACH_PLUGIN_SOURCE_KEY, REACH_POSTERS_KEY, REACH_REGISTRY_KEY,
    REACH_UPDATES_KEY, REACH_USENET_KEY, SWITCHES,
};
use lemonfiber_core::stack::Source;

fn ctx(stack: Source, settings: Settings) -> Ctx {
    lemonfiber_testing::a_live_context()
        .over(stack)
        .settings(settings)
        .build()
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

    assert_eq!(report.ours.len(), 10);
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
        REACH_HOUSEHOLD_KEY,
        REACH_UPDATES_KEY,
        REACH_PLUGIN_SOURCE_KEY,
        REACH_CATALOGUE_KEY,
        REACH_POSTERS_KEY,
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
        unreachable!("the registry is one of the six")
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

/// The one request that carries somebody's words says where they go and what stops
/// if it is refused.
///
/// The sixth entry is the only one that reaches a person rather than a service, and it
/// is the one an operator is most likely to want off — so what it costs to switch off
/// has to be a sentence rather than a shrug, and where it can reach has to be named
/// before anything is sent.
#[tokio::test]
async fn telling_a_household_member_is_listed_with_where_it_reaches_and_what_it_costs() {
    let report = listed(Settings::default()).await;

    let Some(household) = report
        .ours
        .iter()
        .find(|entry| entry.reach.as_str() == "household")
    else {
        unreachable!("carrying a refusal's reason is one of the six")
    };
    assert_eq!(household.switch, REACH_HOUSEHOLD_KEY);
    assert!(household.allowed, "a fresh install refuses to say why");
    assert_eq!(household.destination.len(), 2, "{household:?}");
    // What travels is the operator's own sentence, so an operator reading this is owed
    // the two things nothing else here has to promise: that the words go nowhere but to
    // the person, and that nothing of this product's own goes with them.
    assert!(household.sends.contains("Why"), "{}", household.sends);
    assert!(
        household.cost.contains("reaches nobody but you"),
        "{}",
        household.cost
    );

    let refused = listed(Settings {
        reaching: Reaching::without(REACH_HOUSEHOLD_KEY),
        ..Settings::default()
    })
    .await;
    assert!(
        refused
            .ours
            .iter()
            .any(|entry| entry.reach.as_str() == "household" && !entry.allowed),
        "the one entry that reaches a person could not be switched off"
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
    let Some(aggregator) = report
        .theirs
        .iter()
        .find(|entry| entry.service == "prowlarr")
    else {
        unreachable!("the stack runs an indexer manager")
    };
    assert!(
        aggregator.destination.contains("indexers"),
        "{aggregator:?}"
    );
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

/// Where a request from this machine may go: every destination lemonfiber is documented
/// to ask, and every one an installed plugin declares it may be sent to on its behalf.
async fn expected(settings: Settings) -> Vec<String> {
    let report = listed(settings).await;
    report
        .ours
        .into_iter()
        .flat_map(|entry| entry.destination)
        .chain(report.on_behalf.into_iter().map(|one| one.destination))
        .collect()
}

/// Whether a request to `url` goes somewhere `expected` names, as a whole address or
/// as a host.
fn expected_at(url: &str, expected: &[String]) -> bool {
    expected
        .iter()
        .any(|known| url.starts_with(known.as_str()) || host(url) == known)
}

/// What a whole non-disruptive diagnosis under `settings` sent off this machine to
/// somewhere it was not expected to go.
///
/// The other guards read source text; this watches. A full run is the widest thing this
/// product does on its own, and every request it makes goes through the one transport,
/// so the transport is where the claim can be observed rather than argued. What counts
/// as expected is not a second list written here: it is the enumeration itself.
async fn unexpected_after_a_diagnosis(settings: Settings) -> Vec<String> {
    let http = lemonfiber_fixtures::http::Fake::silent();
    let ctx = lemonfiber_testing::a_context()
        .runner(Arc::new(lemonfiber_fixtures::ports::Idle))
        .engine(Arc::new(lemonfiber_fixtures::support::Reporting::holding(
            &["jellyfin", "prowlarr", "sabnzbd", "qbittorrent"],
            lemonfiber_core::ports::docker::Lifecycle::Running,
            lemonfiber_core::ports::docker::Health::Healthy,
        )))
        .filesystem(lemonfiber_fixtures::files::Files::ending(Vec::new()))
        .settings(settings.clone())
        .build()
        .with_http(http.clone());

    let ran =
        lemonfiber_core::app::diagnose(&ctx, &lemonfiber_core::doctor::Narrowing::Suite, false)
            .await;
    assert!(ran.is_ok(), "the diagnosis ran: {ran:?}");

    let left: Vec<String> = http
        .requests()
        .into_iter()
        .map(|request| request.url)
        .filter(|url| !stays_here(url))
        .collect();
    assert!(
        !left.is_empty(),
        "nothing in this run left the machine at all, so the claim is about nothing"
    );
    let expected = expected(settings).await;
    left.into_iter()
        .filter(|url| !expected_at(url, &expected))
        .collect()
}

/// Nothing unexpected leaves this machine during a whole diagnosis with no plugin
/// installed, and with none installed what is expected is exactly what lemonfiber is
/// documented to ask.
#[tokio::test]
async fn a_whole_diagnosis_reaches_nowhere_the_operator_was_not_told_about() {
    let unexpected = unexpected_after_a_diagnosis(Settings::default()).await;
    assert!(
        unexpected.is_empty(),
        "these left this machine and the operator is told about none of them: {unexpected:?}"
    );

    let documented: Vec<String> = listed(Settings::default())
        .await
        .ours
        .into_iter()
        .flat_map(|entry| entry.destination)
        .collect();
    assert!(!documented.is_empty(), "nothing is documented at all");
    assert_eq!(expected(Settings::default()).await, documented);
}

/// With a plugin installed, a whole diagnosis is measured against the documented set
/// together with what that plugin declares it reaches, and only then does its reach
/// count as expected.
#[tokio::test]
async fn a_diagnosis_with_a_plugin_installed_is_measured_against_its_declared_reach_too() {
    let with = beside("leak-plugin", INSTALLED);

    let unexpected = unexpected_after_a_diagnosis(with.clone()).await;
    assert!(
        unexpected.is_empty(),
        "these left this machine and the operator is told about none of them: {unexpected:?}"
    );

    let declared = "https://metadata.example/v1/series";
    assert!(expected_at(declared, &expected(with).await));
    assert!(!expected_at(declared, &expected(Settings::default()).await));
}

/// The host a URL names, without its port or path.
fn host(url: &str) -> &str {
    url.split_once("://")
        .map_or(url, |(_, rest)| rest)
        .split('/')
        .next()
        .unwrap_or_default()
        .split(':')
        .next()
        .unwrap_or_default()
}

/// Whether a URL names somewhere only this machine can reach.
///
/// The stack's own services are reached by container name on Compose's network or by
/// a loopback port, and neither is a request that leaves.
fn stays_here(url: &str) -> bool {
    let host = host(url);
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
    let at = lemonfiber_fixtures::scratch::Scratch::named(&format!("outbound-{}.log", line!()));
    let _ = std::fs::remove_file(&at);

    let answering: Arc<dyn lemonfiber_core::ports::http::Http> =
        lemonfiber_fixtures::http::Fake::always(lemonfiber_fixtures::http::Answer::Reply(
            204,
            String::new(),
        ));
    let held = ctx(Source::External(project()), Settings::default())
        .with_http(answering)
        .recording_at(at.to_path_buf());
    let transport = Arc::clone(&held.seams.http);

    let asked = lemonfiber_core::ports::http::Request {
        method: lemonfiber_core::ports::http::Method::Get,
        url: "https://indexer.example/api?apikey=the-indexer-key".to_owned(),
        headers: vec![("X-Api-Key".to_owned(), "the-indexer-key".to_owned())],
        body: None,
        pinned: None,
    };
    let answered = transport.send(&asked).await;

    let written = std::fs::read_to_string(&at).unwrap_or_default();
    let _ = std::fs::remove_file(&at);

    assert!(answered.is_ok(), "the request still happened");
    assert!(
        written.contains("indexer.example"),
        "where the request went was not written down"
    );
    assert!(
        written.contains("204"),
        "what came back was not written down"
    );
    assert!(
        !written.contains("the-indexer-key"),
        "the indexer key was written down, in the URL or the header"
    );
}

/// A settings file for the named case with the given register kept beside it, the
/// way an install leaves one.
fn beside(name: &str, register: &str) -> Settings {
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("outbound-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        std::fs::create_dir_all(&dir).is_ok(),
        "the scratch directory"
    );
    assert!(
        std::fs::write(dir.join("plugins.json"), register).is_ok(),
        "the register"
    );
    Settings {
        env_file: Some(dir.join(".env")),
        ..Settings::default()
    }
}

/// One installed plugin whose recipes reach a host outside the stack.
const INSTALLED: &str = r#"{
  "installed": [
    {
      "plugin": "comics",
      "version": "1.2.0",
      "services": [
        {
          "service": "komga",
          "image": "docker.io/gotson/komga",
          "digest": "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
          "tag": "1.11.0",
          "config_path": "/config",
          "takes_data": true
        }
      ],
      "declared": { "reaches": ["metadata.example"] }
    }
  ]
}"#;

/// What an installed plugin brings is in the same account as everything else, and
/// every line of it says whose it is.
#[tokio::test]
async fn what_a_plugin_brings_is_in_the_account_and_says_it_is_the_plugins() {
    let report = listed(beside("plugin", INSTALLED)).await;

    let services: Vec<&str> = report
        .theirs
        .iter()
        .filter(|one| {
            one.origin
                == lemonfiber_core::origin::Origin::Plugin {
                    named: "comics".to_owned(),
                }
        })
        .map(|one| one.service.as_str())
        .collect();
    assert_eq!(services, vec!["komga"], "{:?}", report.theirs);
    let sent: Vec<(&str, &str)> = report
        .on_behalf
        .iter()
        .map(|one| (one.plugin.as_str(), one.destination.as_str()))
        .collect();
    assert_eq!(sent, vec![("comics", "metadata.example")]);
}

/// A register that is there and will not read refuses the account rather than being
/// read past: one that quietly left a stranger's plugin out would be believed.
#[tokio::test]
async fn a_register_that_will_not_read_refuses_the_account() {
    let answered = dispatch(
        Command::Outbound,
        &ctx(
            Source::External(project()),
            beside("broken", "{ not a register"),
        ),
    )
    .await;

    assert_eq!(
        answered.err().map(|problem| problem.code.to_string()),
        Some("PLUGIN-4".to_owned())
    );
}
