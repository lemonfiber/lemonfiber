use std::net::IpAddr;

use super::*;

fn at(url: &str) -> Address {
    Address {
        url: url.to_owned(),
        caution: None,
    }
}

fn ip(written: &str) -> Option<IpAddr> {
    written.parse().ok()
}

#[test]
fn a_request_from_this_machine_or_the_households_network_is_from_home() {
    for from in [
        None,
        ip("127.0.0.1"),
        ip("192.168.1.20"),
        ip("10.0.0.4"),
        ip("172.16.3.3"),
        ip("169.254.1.1"),
        ip("::1"),
        ip("fd12::4"),
        ip("fe80::1"),
        ip("::ffff:192.168.1.20"),
    ] {
        assert_eq!(whence(from), Whence::Home, "{from:?}");
    }
}

#[test]
fn a_request_from_anywhere_else_is_from_away_but_the_shared_carrier_range_is_unknown() {
    assert_eq!(whence(ip("203.0.113.7")), Whence::Away);
    assert_eq!(whence(ip("2001:db8::7")), Whence::Away);
    assert_eq!(whence(ip("100.64.0.1")), Whence::Unknown);
    assert_eq!(whence(ip("100.127.255.254")), Whence::Unknown);
    assert_eq!(whence(ip("100.128.0.1")), Whence::Away);
    assert_eq!(whence(ip("100.63.255.255")), Whence::Away);
}

#[test]
fn an_address_on_the_households_network_reaches_home_and_not_away() {
    for url in [
        "http://192.168.1.5:5055",
        "http://10.1.2.3:8096/web",
        "http://169.254.0.9:80",
        "http://[fd00::5]:8096",
        "http://[fe80::5]:8096",
        "http://media.local:8096",
        "http://Media.LOCAL.:8096",
        "http://nas:8096",
    ] {
        let address = at(url);
        assert_eq!(
            reaches(Whence::Home, Some(&address)),
            Reaches::Here,
            "{url}"
        );
        assert_eq!(
            reaches(Whence::Away, Some(&address)),
            Reaches::NotHere,
            "{url}"
        );
        assert_eq!(
            reaches(Whence::Unknown, Some(&address)),
            Reaches::Unknown,
            "{url}"
        );
    }
}

#[test]
fn any_other_address_or_none_is_unknown_from_anywhere() {
    for url in [
        "https://requests.example.com",
        "http://203.0.113.9:8096",
        "http://[2001:db8::9]:8096",
        "not an address",
    ] {
        let address = at(url);
        for from in [Whence::Home, Whence::Away, Whence::Unknown] {
            assert_eq!(reaches(from, Some(&address)), Reaches::Unknown, "{url}");
        }
    }
    assert_eq!(reaches(Whence::Home, None), Reaches::Unknown);
    assert_eq!(reaches(Whence::Away, None), Reaches::Unknown);
}
