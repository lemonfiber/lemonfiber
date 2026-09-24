use std::time::Duration;

use super::waited;

/// Under a second is said in milliseconds. A service that refused instantly and
/// one that took most of a second are different facts, and "0s" is neither.
#[test]
fn a_wait_under_a_second_is_said_in_milliseconds() {
    assert_eq!(waited(Duration::from_millis(0)), "0ms");
    assert_eq!(waited(Duration::from_millis(937)), "937ms");
}

/// A second or more is said in seconds to a tenth — the precision that separates
/// a slow service from one that ran all the way to the bound.
#[test]
fn a_wait_of_a_second_or_more_is_said_in_seconds() {
    assert_eq!(waited(Duration::from_millis(1000)), "1.0s");
    assert_eq!(waited(Duration::from_millis(9450)), "9.4s");
    assert_eq!(waited(Duration::from_secs(30)), "30.0s");
}
