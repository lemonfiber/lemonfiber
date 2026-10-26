//! The provider check, driven end to end through the diagnosis it is part of.
//!
//! From here rather than from a `#[cfg(test)]` module for the reason the
//! credentials check is: the whole path is `#[async_trait]` clients built on
//! another, which is compiled twice and whose coverage is counted from the wrong
//! copy when it is exercised in-crate. Driving it from outside also proves the
//! part an in-crate test cannot — that a real stack's services resolve to the two
//! clients this reads, at the addresses and config paths the manifest declares.
//!
//! The stack is the real one, read from the repository's own embedded copy, with its
//! Usenet client and its indexer aggregator moved into their plugins. The services under
//! it are fakes: a transport answering as the Usenet client's adapter and the
//! aggregator's would.

use std::sync::Arc;

use lemonfiber_core::app::{diagnose, Ctx};
use lemonfiber_core::config::Settings;
use lemonfiber_core::doctor::{Category, Narrowing, Verdict};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::service::{IndexerUse, Recorded, Standing, UsenetAccount};
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_testing::adapted::{answered, path_of};
use lemonfiber_testing::indexer::INDEXER;
use lemonfiber_testing::usenet;

/// A gibibyte.
const GIB: u64 = 1024 * 1024 * 1024;

/// One Usenet account with a 500 GiB block recorded against it, ten of which the client
/// has measured it pulling, standing as the client last found it.
fn a_block(standing: Standing) -> UsenetAccount {
    UsenetAccount {
        name: "Block 500".to_owned(),
        enabled: true,
        quota: Some(Recorded {
            cap: 500 * GIB,
            from: 0,
        }),
        downloaded: 10 * GIB,
        daily: Vec::new(),
        expires_on: None,
        standing: Some(standing),
    }
}

/// How the client is finding that account right now: in rotation, half its connections
/// up, and nothing recorded against it.
fn serving() -> Standing {
    Standing {
        ready: 4,
        configured: 8,
        serving: true,
        trouble: None,
    }
}

/// The same account, refusing the credentials the client offers it — the provider's own
/// words, as the client wrote them down.
fn refusing() -> Standing {
    Standing {
        ready: 0,
        configured: 8,
        serving: false,
        trouble: Some(
            "Failed login for server news.example.com [481 Authentication rejected]".to_owned(),
        ),
    }
}

/// One indexer the aggregator is querying, with today's counts.
fn fast_indexer() -> IndexerUse {
    IndexerUse {
        name: "Fast Indexer".to_owned(),
        enabled: true,
        queries: 12,
        failed_queries: 0,
        grabs: 2,
        failed_grabs: 0,
        rested_until: None,
        limits: None,
        searched_from: None,
        grabbed_from: None,
    }
}

/// A context over the real stack, its Usenet client and its indexer aggregator moved into
/// their plugins, the client's adapter answering `download.usenet`'s accounts with
/// `account` and the aggregator's answering what it queries with [`fast_indexer`], under an
/// env file of its own named `name`.
fn reaching(name: &str, account: UsenetAccount) -> Ctx {
    use lemonfiber_contract::capabilities::indexer::search;
    let http = Fake::by_path(vec![
        (usenet::operation("accounts"), answered(&vec![account])),
        (
            path_of(search::CAPABILITY, search::MAJOR, "indexers"),
            answered(&vec![fast_indexer()]),
        ),
    ]);
    let dir = lemonfiber_fixtures::scratch::Scratch::named(&format!("providers-{name}")).kept();
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = lemonfiber_testing::a_context()
        .runner(Arc::new(lemonfiber_fixtures::ports::Idle))
        .settings(Settings {
            env_file: Some(dir.join(".env")),
            ..Settings::default()
        })
        .filesystem(Files::empty())
        .build()
        .with_http(http);
    let ctx = lemonfiber_testing::adapted::asking(ctx, &[usenet::MOVED, INDEXER]);
    for moved in [usenet::MOVED, INDEXER] {
        moved.recorded(&ctx);
    }
    ctx
}

#[tokio::test]
async fn the_accounts_behind_a_real_stack_are_read_from_the_services_that_use_them() {
    let ctx = reaching("serving", a_block(serving()));

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
    let ctx = reaching("refusing", a_block(refusing()));

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
    let ctx = lemonfiber_testing::a_context()
        .runner(Arc::new(lemonfiber_fixtures::ports::Idle))
        .engine(Arc::new(Reporting::holding(
            &[],
            Lifecycle::Exited,
            Health::None,
        )))
        .filesystem(Files::empty())
        .build()
        .with_http(Fake::silent());

    let report = diagnose(&ctx, &Narrowing::Category(Category::Providers), false).await;
    let findings = report.map(|report| report.findings).unwrap_or_default();

    // The diagnosis produced something to judge. `unwrap_or_default` hands back an
    // empty list where it failed, and an `all` over that is true — so the claim below
    // would hold most firmly in exactly the case it is meant to catch.
    assert!(
        !findings.is_empty(),
        "the providers diagnosis reported nothing at all"
    );
    assert!(
        findings
            .iter()
            .all(|finding| matches!(finding.verdict, Verdict::Skipped { .. })),
        "nothing to read is skipped, not failed: {findings:?}"
    );
}
