use super::super::fixtures::Recording;
use super::Walk;
use crate::test_support::a_context;
use crate::walkthrough::Speed;

/// A download that has not moved, as the client last reported it.
const STALLED: Speed = Speed {
    total: 4_000_000_000,
    left: 2_000_000_000,
    rate: 0,
};

/// A poll that found nothing new says nothing.
///
/// A download is waited on by asking the client every couple of seconds, for as
/// long as an operator's patience allows. A walk that said each poll's line would
/// bury what it found under how often it looked — and redirected, that is one
/// sentence written out hundreds of times where a reader wanted the two lines
/// either side of it.
#[test]
fn a_poll_that_found_the_same_figures_says_them_once() {
    let ctx = a_context().build();
    let recording = Recording::default();
    let mut walk = Walk::new(&ctx, &recording);

    for _ in 0..4 {
        walk.say_if_new(STALLED.line());
    }

    let said = recording.lines();
    assert_eq!(said.len(), 1, "four polls that agreed: {said:?}");
}

/// And a poll that found something new says it.
///
/// The other half, because a rule that swallowed everything would pass the test
/// above and leave an operator watching a stack that never says anything.
#[test]
fn a_poll_that_found_something_new_says_it() {
    let ctx = a_context().build();
    let recording = Recording::default();
    let mut walk = Walk::new(&ctx, &recording);

    walk.say_if_new(STALLED.line());
    walk.say_if_new(
        Speed {
            rate: 5_000_000,
            ..STALLED
        }
        .line(),
    );

    let said = recording.lines();
    assert_eq!(said.len(), 2, "a download that started moving: {said:?}");
}
