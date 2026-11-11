use lemonfiber_ports::random::Random as _;

use super::Os;

#[test]
fn it_gives_as_many_bytes_as_asked_and_not_the_same_twice() {
    let (one, other) = (Os.bytes(32), Os.bytes(32));
    assert_eq!(one.as_ref().map(Vec::len), Some(32));
    assert_ne!(one, other);
}
