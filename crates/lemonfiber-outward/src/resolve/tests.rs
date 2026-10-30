use std::net::{IpAddr, Ipv4Addr};

use lemonfiber_ports::resolve::Resolver as _;

use super::Lookup;

#[tokio::test]
async fn an_address_written_as_one_stands_for_itself() {
    let found = Lookup.addresses("192.0.2.7", 443).await;
    assert_eq!(found, Ok(vec![IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7))]));
}

/// A name under `.invalid` stands for nothing anywhere, so the resolver's refusal is
/// what comes back rather than an empty list.
#[tokio::test]
async fn a_name_that_stands_for_nothing_says_why() {
    let found = Lookup.addresses("nothing.invalid", 443).await;
    assert!(
        found.as_ref().is_err_and(|why| !why.is_empty()),
        "{found:?}"
    );
}
