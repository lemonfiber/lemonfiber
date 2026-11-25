use std::time::Duration;

use super::spell_out;

#[test]
fn a_wait_is_told_in_the_coarsest_unit_that_still_says_something() {
    assert_eq!(spell_out(Duration::from_secs(45)), "45s");
    assert_eq!(spell_out(Duration::from_secs(120)), "2m");
    assert_eq!(spell_out(Duration::from_secs(7200)), "2h");
}
