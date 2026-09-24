use super::{Fault, Space};

#[test]
fn a_capture_fits_only_when_the_headroom_is_left_over() {
    let space = Space {
        needed: 100,
        available: 250,
    };
    assert!(space.fits(150), "250 holds 100 with 150 to spare");
    assert!(!space.fits(151), "250 cannot hold 100 and 151 more");
}

#[test]
fn a_headroom_that_would_overflow_does_not_wrap_into_fitting() {
    // needed + headroom must saturate rather than wrap past the available
    // figure and read as room where there is none: a gigantic estimate on a
    // small disk does not fit, however large the headroom added to it.
    let space = Space {
        needed: u64::MAX,
        available: 1_000,
    };
    assert!(!space.fits(1));
}

#[test]
fn a_fault_keeps_the_platforms_own_words() {
    assert_eq!(Fault::new("disk full").message, "disk full");
}
