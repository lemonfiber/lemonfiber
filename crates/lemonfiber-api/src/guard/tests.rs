use super::{host_is_here, origin_is_here, Binding, Token, TOKEN_HEADER, WIDTH};
use lemonfiber_fixtures::ports::Chance;

/// The byte every test mints from, chosen because its hex is two of one digit,
/// which makes the token it is written as a repeat rather than a literal.
const EVERY_BYTE: u8 = 0xab;
const AS_HEX: &str = "ab";

/// The token those bytes are written as, built rather than spelled out.
fn written() -> String {
    AS_HEX.repeat(WIDTH)
}

/// Serving this machine and nowhere else.
fn bound() -> Binding {
    Binding::here(8471)
}

/// The same port, offered past this machine.
fn beyond() -> Binding {
    Binding {
        port: 8471,
        beyond: true,
    }
}

/// The source every token test is minted from, answering in full.
fn given() -> Chance {
    Chance::exactly(Some(vec![EVERY_BYTE; WIDTH]))
}

#[test]
fn the_header_is_the_one_both_sides_agreed_on() {
    assert_eq!(TOKEN_HEADER, "X-Lemonfiber-Token");
}

#[test]
fn a_token_is_the_bytes_written_as_hex() {
    assert_eq!(
        Token::mint(&given()).map(|token| token.as_str().to_owned()),
        Some(written())
    );
}

#[test]
fn a_token_is_as_wide_as_the_secret_is_meant_to_be() {
    // Two hex digits a byte. Asserted rather than assumed, because nothing
    // about a short token looks wrong.
    assert_eq!(
        Token::mint(&given()).map(|token| token.as_str().len()),
        Some(WIDTH * 2)
    );
}

#[test]
fn there_is_no_token_when_the_system_will_not_say() {
    assert!(Token::mint(&Chance::exactly(None)).is_none());
}

#[test]
fn there_is_no_token_when_the_system_says_less_than_it_was_asked() {
    let short = Chance::exactly(Some(vec![EVERY_BYTE; WIDTH - 1]));
    assert!(Token::mint(&short).is_none());
}

#[test]
fn a_request_carrying_the_token_is_recognised() {
    assert!(Token::mint(&given()).is_some_and(|token| token.carried_by(Some(&written()))));
}

#[test]
fn a_request_carrying_something_else_is_not() {
    let wrong = written().replace('a', "b");
    assert!(!Token::mint(&given()).is_some_and(|token| token.carried_by(Some(&wrong))));
}

#[test]
fn a_request_carrying_a_prefix_is_not() {
    // One byte's worth of the token, which every longer one begins with.
    assert!(!Token::mint(&given()).is_some_and(|token| token.carried_by(Some(AS_HEX))));
}

#[test]
fn a_request_carrying_nothing_is_not() {
    assert!(!Token::mint(&given()).is_some_and(|token| token.carried_by(None)));
}

#[test]
fn a_host_naming_this_address_is_here() {
    assert!(host_is_here(Some("127.0.0.1:8471"), bound()));
    assert!(host_is_here(Some("localhost:8471"), bound()));
    assert!(host_is_here(Some("[::1]:8471"), bound()));
    assert!(host_is_here(Some("stack.localhost:8471"), bound()));
}

#[test]
fn a_host_naming_another_port_is_not() {
    assert!(!host_is_here(Some("localhost:9000"), bound()));
}

#[test]
fn a_host_naming_somewhere_else_is_not() {
    assert!(!host_is_here(Some("example.com:8471"), bound()));
}

#[test]
fn a_host_carrying_no_port_is_not() {
    assert!(!host_is_here(Some("localhost"), bound()));
}

#[test]
fn a_host_carrying_something_that_is_not_a_port_is_not() {
    assert!(!host_is_here(Some("localhost:doorway"), bound()));
}

#[test]
fn a_request_without_a_host_is_refused() {
    assert!(!host_is_here(None, bound()));
}

#[test]
fn an_origin_naming_this_address_is_here() {
    assert!(origin_is_here(Some("http://localhost:8471"), bound()));
    assert!(origin_is_here(Some("127.0.0.1:8471"), bound()));
}

#[test]
fn an_origin_naming_somewhere_else_is_not() {
    assert!(!origin_is_here(Some("http://evil.example:8471"), bound()));
}

#[test]
fn a_request_stating_no_origin_is_allowed() {
    assert!(origin_is_here(None, bound()));
}

/// Offered past this machine, an address is let through and a name is not.
///
/// There is no list of the addresses this machine answers on — they vary by
/// machine, change with the network, and are different again on a laptop that
/// moves — so what is held is the shape of the word. A rebinding attack needs a
/// name, because what it rebinds is a name; an address typed into a browser is
/// not one and cannot be made to resolve anywhere.
#[test]
fn offered_to_a_network_an_address_reaches_it_and_a_name_still_does_not() {
    assert!(host_is_here(Some("192.168.1.10:8471"), beyond()));
    assert!(host_is_here(Some("[fe80::1]:8471"), beyond()));
    assert!(host_is_here(Some("localhost:8471"), beyond()));
    assert!(!host_is_here(Some("lemonfiber.local:8471"), beyond()));
    assert!(!host_is_here(Some("evil.example:8471"), beyond()));
    // And the port is still held, whichever address named it.
    assert!(!host_is_here(Some("192.168.1.10:9000"), beyond()));
    // On this machine, an address off loopback reaches nothing at all.
    assert!(!host_is_here(Some("192.168.1.10:8471"), bound()));
}
