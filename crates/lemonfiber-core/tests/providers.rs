//! The provider check, driven end to end through the diagnosis it is part of.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the
//! credentials check is: the whole path is `#[async_trait]` clients built on
//! another, which is compiled twice and whose coverage is counted from the wrong
//! copy when it is exercised in-crate. Driving it from outside also proves the
//! part an in-crate test cannot — that a real stack's services resolve to the two
//! clients this reads, at the addresses and config paths the manifest declares.
//!
//! The stack is the real one, read from the repository's own embedded copy. The
//! services under it are fakes: a filesystem holding the two configuration files
//! the clients write their keys to, and a transport answering as the download
//! client and the aggregator would.

mod common;

use common::stack::project;
use std::sync::Arc;

use lemonfiber_core::app::{diagnose, Ctx};
use lemonfiber_core::config::Settings;
use lemonfiber_core::doctor::{Category, Narrowing, Verdict};
use lemonfiber_core::platform::Environment;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::{Answer, Fake};
use lemonfiber_fixtures::support::Reporting;

/// `SABnzbd`'s configuration, carrying the key it generated for itself.
const SAB_INI: &str = "[misc]\napi_key = sabkey123\n";

/// The aggregator's configuration, carrying the key it generated for itself.
const PROWLARR_XML: &str = "<Config><ApiKey>a1b2c3d4e5</ApiKey></Config>";

/// One Usenet account with a block recorded against it, and one day of history.
const SERVERS: &str = r#"{"config":{"servers":[
    {"name":"news.example.com","displayname":"Block 500","enable":1,"quota":"500 G",
     "usage_at_start":0,"expire_date":""}
]}}"#;

/// What that account has pulled, as the client has measured it.
const STATS: &str = r#"{"servers":{"news.example.com":{"total":10737418240,"daily":{}}}}"#;

/// How the client is finding that account right now: in rotation, half its connections
/// up, and nothing recorded against it.
const STATUS: &str = r#"{"status":{"servers":[
    {"servername":"Block 500","serveractive":true,"serveractiveconn":4,"servertotalconn":8,
     "servererror":""}
]}}"#;

/// The same account, refusing the credentials the client offers it — the provider's own
/// words, as the client wrote them down.
const REFUSING: &str = r#"{"status":{"servers":[
    {"servername":"Block 500","serveractive":false,"serveractiveconn":0,"servertotalconn":8,
     "servererror":"Failed login for server news.example.com [481 Authentication rejected]"}
]}}"#;

/// One indexer the aggregator is querying, with today's counts.
const INDEXERS: &str = r#"[{"id":1,"name":"Fast Indexer","enable":true,"status":null}]"#;
const STANDINGS: &str = "[]";
const COUNTS: &str = r#"{"indexers":[{"indexerId":1,"numberOfQueries":12,"numberOfGrabs":2,
    "numberOfFailedQueries":0,"numberOfFailedGrabs":0}]}"#;

#[tokio::test]
async fn the_accounts_behind_a_real_stack_are_read_from_the_services_that_use_them() {
    let http = Fake::by_path(vec![
        ("mode=get_config", Answer::reply(200, SERVERS)),
        ("mode=server_stats", Answer::reply(200, STATS)),
        ("mode=fullstatus", Answer::reply(200, STATUS)),
        ("/api/v1/indexerstatus", Answer::reply(200, STANDINGS)),
        ("/api/v1/indexerstats", Answer::reply(200, COUNTS)),
        ("/api/v1/indexer", Answer::reply(200, INDEXERS)),
    ]);
    let ctx = Ctx::new(
        Arc::new(lemonfiber_fixtures::ports::Idle),
        Arc::new(Reporting::holding(&[], Lifecycle::Exited, Health::None)),
        lemonfiber_fixtures::ports::Stopped::today(),
        Files::ending(vec![
            ("config/sabnzbd/sabnzbd.ini", SAB_INI),
            ("config/prowlarr/config.xml", PROWLARR_XML),
        ]),
        Source::External(project()),
        Settings::default(),
        Environment::MacOs,
    )
    .with_http(http);

    let report = diagnose(&ctx, &Narrowing::Category(Category::Providers), false).await;
    let findings = report.map(|report| report.findings).unwrap_or_default();

    assert!(
        findings
            .iter()
            .any(|finding| finding.title == "Block 500"
                && matches!(&finding.verdict, Verdict::Pass { note }
                    if note.as_deref().is_some_and(|note| note.contains("490.0 GiB left of 500.0 GiB")))),
        "the Usenet account reports what its client measured: {findings:?}"
    );
    assert!(
        findings
            .iter()
            .any(|finding| finding.title == "Fast Indexer"
                && matches!(&finding.verdict, Verdict::Pass { note }
                    if note.as_deref().is_some_and(|note| note.contains("12 searches, 2 grabs")))),
        "the indexer reports what the aggregator counted: {findings:?}"
    );
}

/// The state the whole live view exists to name. Every service is up, the stack is
/// perfect, and the account the downloads run through is turning the client away — which
/// from the outside is indistinguishable from software that has stopped working.
#[tokio::test]
async fn an_account_refusing_the_login_fails_through_the_whole_diagnosis() {
    let http = Fake::by_path(vec![
        ("mode=get_config", Answer::reply(200, SERVERS)),
        ("mode=server_stats", Answer::reply(200, STATS)),
        ("mode=fullstatus", Answer::reply(200, REFUSING)),
        ("/api/v1/indexerstatus", Answer::reply(200, STANDINGS)),
        ("/api/v1/indexerstats", Answer::reply(200, COUNTS)),
        ("/api/v1/indexer", Answer::reply(200, INDEXERS)),
    ]);
    let ctx = Ctx::new(
        Arc::new(lemonfiber_fixtures::ports::Idle),
        Arc::new(Reporting::holding(&[], Lifecycle::Exited, Health::None)),
        lemonfiber_fixtures::ports::Stopped::today(),
        Files::ending(vec![
            ("config/sabnzbd/sabnzbd.ini", SAB_INI),
            ("config/prowlarr/config.xml", PROWLARR_XML),
        ]),
        Source::External(project()),
        Settings::default(),
        Environment::MacOs,
    )
    .with_http(http);

    let report = diagnose(&ctx, &Narrowing::Category(Category::Providers), false).await;
    let findings = report.map(|report| report.findings).unwrap_or_default();

    assert!(
        findings.iter().any(|finding| finding.title == "Block 500"
            && matches!(&finding.verdict, Verdict::Fail(problem)
                if problem.detail.as_deref().is_some_and(|detail| detail.contains("481")))),
        "a rejected login is reported as one, in the provider's own words: {findings:?}"
    );
}

/// A stack whose services have not written their keys yet has nothing to read —
/// which is a later run's business, not a fault.
#[tokio::test]
async fn a_stack_whose_services_have_not_started_reports_nothing_to_read() {
    let ctx = Ctx::new(
        Arc::new(lemonfiber_fixtures::ports::Idle),
        Arc::new(Reporting::holding(&[], Lifecycle::Exited, Health::None)),
        lemonfiber_fixtures::ports::Stopped::today(),
        Files::empty(),
        Source::External(project()),
        Settings::default(),
        Environment::MacOs,
    )
    .with_http(Fake::silent());

    let report = diagnose(&ctx, &Narrowing::Category(Category::Providers), false).await;
    let findings = report.map(|report| report.findings).unwrap_or_default();

    assert!(
        findings
            .iter()
            .all(|finding| matches!(finding.verdict, Verdict::Skipped { .. })),
        "nothing to read is skipped, not failed: {findings:?}"
    );
}
