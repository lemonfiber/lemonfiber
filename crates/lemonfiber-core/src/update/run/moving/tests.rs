use super::{held, UNRECORDED};

/// An install that answered with nothing to record took no move, and says so.
#[test]
fn an_install_answering_with_nothing_to_record_took_no_move() {
    assert_eq!(held(None), Err(UNRECORDED.to_owned()));
}
