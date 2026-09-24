use super::Os;
use lemonfiber_ports::random::Random;

#[test]
fn it_returns_the_requested_number_of_bytes() {
    let bytes = Os.bytes(24);
    assert_eq!(bytes.map(|bytes| bytes.len()), Some(24));
}

#[test]
fn two_draws_do_not_come_out_the_same() {
    // Not a statistical test — just that the source is not a fixed constant.
    // Two 24-byte draws colliding is far past astronomically unlikely.
    assert_ne!(Os.bytes(24), Os.bytes(24));
}
