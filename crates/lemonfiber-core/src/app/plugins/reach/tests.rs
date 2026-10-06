//! Where a git source may be fetched from.

use std::net::IpAddr;

use lemonfiber_fixtures::ports::Resolving;

use super::{host_of, reached, refused, Reached};
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

/// This machine, a network of its own and no address at all are refused, an IPv4
/// address carried inside an IPv6 one included; an address out on the internet is not.
#[test]
fn an_address_here_or_on_a_network_of_its_own_is_refused() {
    for written in [
        "127.0.0.1",
        "127.8.9.10",
        "10.0.0.5",
        "172.16.0.1",
        "192.168.1.20",
        "169.254.169.254",
        "0.0.0.0",
        "::1",
        "::",
        "fd00::1",
        "fc12:3456::1",
        "fe80::1",
        "febf::1",
        "::ffff:127.0.0.1",
        "::ffff:10.1.2.3",
        "::ffff:169.254.169.254",
    ] {
        assert!(refused(ip(written)), "{written} was let through");
    }
    for written in [
        "203.0.113.10",
        "8.8.8.8",
        "172.32.0.1",
        "2001:db8::1",
        "2606:4700::1111",
        "fec0::1",
        "::ffff:8.8.8.8",
    ] {
        assert!(!refused(ip(written)), "{written} was refused");
    }
}

/// The host and port are read past a user, from brackets, and up to whatever ends them.
#[test]
fn the_host_is_read_past_everything_around_it() {
    let read = |url: &str| host_of(url);
    assert_eq!(
        read("https://example.org/plugin"),
        Some(("example.org".to_owned(), 443))
    );
    assert_eq!(
        read("https://example.org"),
        Some(("example.org".to_owned(), 443))
    );
    assert_eq!(
        read("https://git@example.org:8443/x"),
        Some(("example.org".to_owned(), 8443))
    );
    assert_eq!(
        read("https://a:b@c@example.org?x"),
        Some(("example.org".to_owned(), 443))
    );
    assert_eq!(
        read("https://[fd00::1]:444/x"),
        Some(("fd00::1".to_owned(), 444))
    );
    assert_eq!(read("https://[::1]/x"), Some(("::1".to_owned(), 443)));
    assert_eq!(
        read("https://example.org#top"),
        Some(("example.org".to_owned(), 443))
    );
    for nameless in [
        "https:///x",
        "https://user@/x",
        "https://example.org:port/x",
        "https://[::1/x",
        "http://example.org/x",
    ] {
        assert_eq!(read(nameless), None, "{nameless}");
    }
}

/// A name standing only for addresses out on the internet is reached, and git is
/// handed those addresses as the only answer for it.
#[tokio::test]
async fn a_name_out_on_the_internet_is_pinned_to_what_it_stood_for() {
    let resolving = Resolving::standing_for(&[ip("203.0.113.10"), ip("2001:db8::1")]);
    let ctx = a_context().build().with_resolver(resolving.clone());

    let outcome = reached(&ctx, "https://example.org:8443/plugin").await;

    assert_eq!(
        outcome
            .ok()
            .and_then(|reached| reached.pin().map(str::to_owned))
            .as_deref(),
        Some("http.curloptResolve=example.org:8443:203.0.113.10,[2001:db8::1]")
    );
    assert_eq!(resolving.asked(), [("example.org".to_owned(), 8443)]);
}

/// A name standing for anything here is refused, even beside an address out there.
#[tokio::test]
async fn a_name_standing_for_anywhere_here_is_refused() {
    let resolving = Resolving::standing_for(&[ip("203.0.113.10"), ip("10.0.0.5")]);
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
    let there = reached(&ctx, "https://203.0.113.10/x").await;

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
