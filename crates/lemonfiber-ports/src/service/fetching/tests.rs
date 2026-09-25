use super::Pulling;

#[test]
fn a_client_is_stopped_only_when_nothing_moves_and_nothing_new_would_start() {
    // Either half alone is a client that goes on fetching: stopping what is
    // running leaves the next grab to start, and refusing new work leaves what
    // is already running to spend the rest of the month.
    assert_eq!(Pulling::of(false, false), Pulling::Stopped);
    assert_eq!(Pulling::of(true, false), Pulling::Fetching);
    assert_eq!(Pulling::of(false, true), Pulling::Fetching);
    assert_eq!(Pulling::of(true, true), Pulling::Fetching);
}
