//! Whether a host stands for somewhere out on the internet.

use std::net::IpAddr;

use lemonfiber_fixtures::ports::Resolving;

use super::{checked, host_of, internal, literal, outward, Inward};
use crate::ports::http::{Method, Request};

/// The address written, read as one.
fn ip(written: &str) -> IpAddr {
    written.parse().unwrap_or(IpAddr::from([0, 0, 0, 0]))
}

/// Every IPv4 class, each at its first and last address, beside the address just
/// outside it, which is out on the internet.
#[test]
fn every_ipv4_class_is_refused_at_both_its_edges_and_nothing_beside_it() {
    for (class, inside, outside) in [
        ("this network", ["0.0.0.0", "0.255.255.255"], "1.0.0.0"),
        ("private", ["10.0.0.0", "10.255.255.255"], "11.0.0.0"),
        ("shared", ["100.64.0.0", "100.127.255.255"], "100.128.0.0"),
        ("loopback", ["127.0.0.1", "127.255.255.255"], "128.0.0.0"),
        (
            "link-local",
            ["169.254.0.0", "169.254.169.254"],
            "169.255.0.0",
        ),
        ("private", ["172.16.0.0", "172.31.255.255"], "172.32.0.0"),
        ("protocol", ["192.0.0.0", "192.0.0.255"], "192.0.1.0"),
        ("documentation", ["192.0.2.0", "192.0.2.255"], "192.0.3.0"),
        ("private", ["192.168.0.0", "192.168.255.255"], "192.169.0.0"),
        (
            "benchmarking",
            ["198.18.0.0", "198.19.255.255"],
            "198.20.0.0",
        ),
        (
            "documentation",
            ["198.51.100.0", "198.51.100.255"],
            "198.51.101.0",
        ),
        (
            "documentation",
            ["203.0.113.0", "203.0.113.255"],
            "203.0.114.0",
        ),
        (
            "multicast",
            ["224.0.0.0", "239.255.255.255"],
            "223.255.255.255",
        ),
        (
            "reserved",
            ["240.0.0.0", "255.255.255.254"],
            "100.63.255.255",
        ),
        (
            "broadcast",
            ["255.255.255.255", "255.255.255.255"],
            "8.8.8.8",
        ),
    ] {
        for written in inside {
            assert!(internal(ip(written)), "{class}: {written} was let through");
        }
        assert!(!internal(ip(outside)), "{class}: {outside} was refused");
    }
}

/// Every IPv6 class, at an address inside it, beside one just outside it.
#[test]
fn every_ipv6_class_is_refused_and_nothing_beside_it() {
    for (class, inside, outside) in [
        ("unspecified", "::", "2a00:1450::1"),
        ("loopback", "::1", "2a00:1450::2"),
        ("discard", "100::ffff", "100:0:0:1::"),
        ("local NAT64", "64:ff9b:1::1", "64:ff9b:2::1"),
        ("benchmarking", "2001:2::1", "2001:3::1"),
        ("documentation", "2001:db8:ffff::1", "2001:db9::1"),
        ("documentation", "3fff:fff::1", "3fff:1000::1"),
        ("unique-local", "fdff::1", "fe00::1"),
        ("unique-local", "fc00::1", "fbff::1"),
        ("link-local", "febf::1", "fe7f::1"),
        ("site-local", "feff::1", "fe00::2"),
        ("multicast", "ff02::1", "2a00::1"),
    ] {
        assert!(internal(ip(inside)), "{class}: {inside} was let through");
        assert!(!internal(ip(outside)), "{class}: {outside} was refused");
    }
}

/// An IPv6 address carrying an IPv4 one is classed by the IPv4 address too, whichever
/// way it carries it; one carrying an address out there is let through.
#[test]
fn an_ipv4_address_carried_inside_an_ipv6_one_is_classed_as_itself() {
    for (how, here, there) in [
        ("mapped", "::ffff:127.0.0.1", "::ffff:8.8.8.8"),
        ("mapped", "::ffff:169.254.169.254", "::ffff:1.1.1.1"),
        ("compatible", "::10.0.0.1", "::8.8.8.8"),
        ("NAT64", "64:ff9b::a9fe:a9fe", "64:ff9b::808:808"),
        ("6to4", "2002:a00:1::1", "2002:808:808::1"),
        (
            "Teredo server",
            "2001:0:a00:1::f7f7:f7f7",
            "2001:0:808:808::f7f7:f7f7",
        ),
        (
            "Teredo client",
            "2001:0:808:808::80ff:fffe",
            "2001:0:808:808::f7f7:f7f7",
        ),
    ] {
        assert!(internal(ip(here)), "{how}: {here} was let through");
        assert!(!internal(ip(there)), "{how}: {there} was refused");
    }
}

/// An address is read as the address it is however it is spelled, and a name is not.
#[test]
fn every_spelling_of_an_address_is_read_as_that_address() {
    for (written, address) in [
        ("127.0.0.1", "127.0.0.1"),
        ("2130706433", "127.0.0.1"),
        ("0x7f000001", "127.0.0.1"),
        ("0X7F000001", "127.0.0.1"),
        ("0177.0.0.1", "127.0.0.1"),
        ("0x7f.0.0.1", "127.0.0.1"),
        ("127.1", "127.0.0.1"),
        ("127.0.1", "127.0.0.1"),
        ("10.65535", "10.0.255.255"),
        ("169.254.43518", "169.254.169.254"),
        ("0", "0.0.0.0"),
        ("00", "0.0.0.0"),
        ("4294967295", "255.255.255.255"),
        ("::1", "::1"),
        ("[::ffff:7f00:1]", "::ffff:127.0.0.1"),
    ] {
        assert_eq!(literal(written), Some(ip(address)), "{written}");
    }
    for name in [
        "example.org",
        "1.2.3.4.5",
        "256.1.1.1",
        "1.256.1",
        "1.2.65536",
        "08.1.1.1",
        "0x",
        "0xg.1",
        "4294967296",
        "1..2",
        "",
        "[::1",
        "-1.2.3.4",
        "+1.2.3.4",
        "1.2.3.+4",
        "0x+1",
    ] {
        assert_eq!(literal(name), None, "{name}");
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
        read("http://example.org/x"),
        Some(("example.org".to_owned(), 80))
    );
    assert_eq!(
        read("https://example.org#top"),
        Some(("example.org".to_owned(), 443))
    );
    for nameless in [
        "https:///x",
        "https://user@/x",
        "https://example.org:port/x",
        "https://[::1/x",
        "ftp://example.org/x",
    ] {
        assert_eq!(read(nameless), None, "{nameless}");
    }
}

/// A name standing only for addresses out there is answered with every one of them,
/// asked of the resolver on the port it is reached on.
#[tokio::test]
async fn a_name_out_on_the_internet_is_answered_with_what_it_stands_for() {
    let resolving = Resolving::standing_for(&[ip("192.88.99.10"), ip("2a00:1450::1")]);

    let reached = outward(&*resolving, "example.org", 443).await;

    assert_eq!(reached, Ok(vec![ip("192.88.99.10"), ip("2a00:1450::1")]));
    assert_eq!(resolving.asked(), [("example.org".to_owned(), 443)]);
}

/// One address here among addresses out there is enough to refuse the name, and the
/// refusal names it.
#[tokio::test]
async fn a_name_standing_for_anywhere_here_is_refused_naming_where() {
    let resolving = Resolving::standing_for(&[ip("192.88.99.10"), ip("10.0.0.5")]);

    let reached = outward(&*resolving, "example.org", 443).await;

    assert_eq!(reached, Err(Inward::Internal(ip("10.0.0.5"))));
}

/// An address written as the host is judged as written in any spelling, and nothing is
/// asked.
#[tokio::test]
async fn an_address_written_as_the_host_is_judged_without_asking() {
    let resolving = Resolving::anywhere();

    assert_eq!(
        outward(&*resolving, "192.88.99.10", 443).await,
        Ok(vec![ip("192.88.99.10")])
    );
    for here in ["::ffff:127.0.0.1", "127.1", "0x7f000001", "[::1]"] {
        assert!(
            matches!(
                outward(&*resolving, here, 443).await,
                Err(Inward::Internal(_))
            ),
            "{here}"
        );
    }
    assert!(resolving.asked().is_empty());
}

/// A resolver that could not answer and a name standing for nothing are told apart.
#[tokio::test]
async fn a_name_that_stands_for_nothing_is_not_reached() {
    assert_eq!(
        outward(&*Resolving::failing("no such host"), "example.org", 443).await,
        Err(Inward::Unresolved("no such host".to_owned()))
    );
    assert_eq!(
        outward(&*Resolving::standing_for(&[]), "example.org", 443).await,
        Err(Inward::Nowhere)
    );
}

/// A call to a name out on the internet.
fn calling(url: &str) -> Request {
    Request {
        method: Method::Get,
        url: url.to_owned(),
        headers: Vec::new(),
        body: None,
        pinned: None,
    }
}

/// A call is held to the addresses its one check passed. A name that answers with an
/// address out there and then with one here never puts the second on a call: the call
/// checked is held to the first, and the next call checks again and is refused.
#[tokio::test]
async fn a_name_rebound_after_its_check_never_reaches_where_it_was_rebound_to() {
    let resolving = Resolving::rebinding(&[ip("192.88.99.10")], &[ip("10.0.0.5")]);

    let first = checked(&*resolving, calling("https://example.org/x")).await;
    let second = checked(&*resolving, calling("https://example.org/x")).await;

    assert_eq!(
        first.map(|request| request.pinned),
        Ok(Some(vec![ip("192.88.99.10")]))
    );
    assert_eq!(second, Err(Inward::Internal(ip("10.0.0.5"))));
    assert_eq!(resolving.asked().len(), 2, "each call asked exactly once");
}

/// A call whose address names no host is not sent.
#[tokio::test]
async fn a_call_naming_no_host_is_not_checked_and_not_sent() {
    let resolving = Resolving::anywhere();
    assert_eq!(
        checked(&*resolving, calling("https://")).await,
        Err(Inward::Nameless)
    );
    assert!(resolving.asked().is_empty());
}

/// The host checked is the one the transport connects as, even where a hand reading of
/// the address would find another: a backslash ends the host, so the name after the `@`
/// is path, never host.
#[tokio::test]
async fn the_host_checked_is_the_one_the_call_connects_as() {
    let resolving = Resolving::standing_for(&[ip("192.88.99.10")]);
    let held = checked(
        &*resolving,
        calling("https://evil.example\\@good.example/x"),
    )
    .await;

    assert!(held.is_ok());
    assert_eq!(resolving.asked(), [("evil.example".to_owned(), 443)]);

    let spelled = checked(&*Resolving::anywhere(), calling("https://0x7f.1/x")).await;
    assert_eq!(spelled, Err(Inward::Internal(ip("127.0.0.1"))));

    let unread = checked(&*Resolving::anywhere(), calling("not an address")).await;
    assert_eq!(unread, Err(Inward::Nameless));
    let hostless = checked(&*Resolving::anywhere(), calling("data:text/plain,x")).await;
    assert_eq!(hostless, Err(Inward::Nameless));
}
