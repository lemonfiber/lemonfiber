use super::{Clock, System};

#[test]
fn reports_a_time_that_moves_forwards() {
    let before = System.now();
    let after = System.now();
    assert!(after >= before);
}
