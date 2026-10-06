//! Where a git source may be fetched from.

use std::net::IpAddr;

use lemonfiber_fixtures::ports::Resolving;

use super::{reached, Reached};
use crate::test_support::a_context;

/// The address written, read as one.
fn ip(written: &str) -> IpAddr {
    written.parse().unwrap_or(IpAddr::from([0, 0, 0, 0]))
}

/// The code a refusal carries, or nothing where it was not one.
fn code(outcome: &Result<Reached, Box<crate::error::Problem>>) -> Option<String> {
    outcome
        .as_ref()
        .err()
        .map(|problem| problem.code.to_string())
}

/// A name standing only for addresses out on the internet is reached, and git is
/// handed those addresses as the only answer for it.
#[tokio::test]
async fn a_name_out_on_the_internet_is_pinned_to_what_it_stood_for() {
    let resolving = Resolving::standing_for(&[ip("192.88.99.10"), ip("2a00:1450::1")]);
    let ctx = a_context().build().with_resolver(resolving.clone());

    let outcome = reached(&ctx, "https://example.org:8443/plugin").await;

    assert_eq!(
        outcome
            .ok()
            .and_then(|reached| reached.pin().map(str::to_owned))
            .as_deref(),
        Some("http.curloptResolve=example.org:8443:192.88.99.10,[2a00:1450::1]")
    );
    assert_eq!(resolving.asked(), [("example.org".to_owned(), 8443)]);
}

/// A name standing for anything here is refused, even beside an address out there.
#[tokio::test]
async fn a_name_standing_for_anywhere_here_is_refused() {
    let resolving = Resolving::standing_for(&[ip("192.88.99.10"), ip("10.0.0.5")]);
    let ctx = a_context().build().with_resolver(resolving);

    let outcome = reached(&ctx, "https://example.org/plugin").await;

    assert_eq!(code(&outcome).as_deref(), Some("PLUGIN-32"));
    assert!(
        outcome
            .err()
            .is_some_and(|problem| problem.meaning.contains("10.0.0.5")),
        "it names the address"
    );
}

/// An address written as the host is judged as it is written, and nothing is asked.
#[tokio::test]
async fn an_address_written_as_the_host_is_judged_without_asking() {
    let resolving = Resolving::anywhere();
    let ctx = a_context().build().with_resolver(resolving.clone());

    let here = reached(&ctx, "https://169.254.169.254/latest").await;
    let mapped = reached(&ctx, "https://[::ffff:127.0.0.1]/x").await;
    let there = reached(&ctx, "https://192.88.99.10/x").await;

    assert_eq!(code(&here).as_deref(), Some("PLUGIN-32"));
    assert_eq!(code(&mapped).as_deref(), Some("PLUGIN-32"));
    assert_eq!(there.ok(), Some(Reached { pin: None }));
    assert!(resolving.asked().is_empty());
}

/// A name that stands for nothing, a resolver that could not answer and an address
/// that names no host are each a source that could not be fetched.
#[tokio::test]
async fn a_host_that_stands_for_nothing_could_not_be_fetched() {
    let failing = a_context()
        .build()
        .with_resolver(Resolving::failing("no such host"));
    let empty = a_context()
        .build()
        .with_resolver(Resolving::standing_for(&[]));

    let unanswered = reached(&failing, "https://example.org/x").await;
    let nowhere = reached(&empty, "https://example.org/x").await;
    let nameless = reached(&empty, "https:///x").await;

    assert_eq!(code(&unanswered).as_deref(), Some("PLUGIN-16"));
    assert_eq!(code(&nowhere).as_deref(), Some("PLUGIN-16"));
    assert_eq!(code(&nameless).as_deref(), Some("PLUGIN-16"));
}
