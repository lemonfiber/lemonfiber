use super::{Cap, Metered, Reached, WhenExceeded, ONLY_THE_STACK, WARN_AT};

/// A hundred-gigabyte month.
fn capped(exceeded: WhenExceeded) -> Cap {
    Cap {
        monthly: 100 * 1024 * 1024 * 1024,
        exceeded,
    }
}

#[test]
fn a_month_is_warned_about_before_it_is_spent() {
    let cap = capped(WhenExceeded::Pause);
    assert_eq!(cap.reached(0), Reached::Within);
    assert_eq!(cap.reached(cap.monthly / 2), Reached::Within);
    assert_eq!(cap.reached(cap.monthly * 9 / 10), Reached::Warning);
    assert_eq!(cap.reached(cap.monthly), Reached::Exceeded);
    assert_eq!(cap.reached(cap.monthly * 2), Reached::Exceeded);
    // The boundary read off the constant rather than a figure written beside it.
    // `WARN_AT < 100` is decided at compile time and proves nothing at run time:
    // it would hold just as well against a `reached` that never warned at all.
    // What the warning being *before* the cap comes to is that a month exists
    // which warns and is not yet exceeded, and this is that month.
    let at_the_warning = cap.monthly * WARN_AT / 100;
    assert_eq!(cap.reached(at_the_warning), Reached::Warning);
    assert_eq!(cap.reached(at_the_warning - 1), Reached::Within);
}

#[test]
fn what_is_left_never_goes_below_nothing() {
    let cap = capped(WhenExceeded::Continue);
    assert_eq!(cap.left(0), cap.monthly);
    assert_eq!(cap.left(cap.monthly * 3), 0);
}

#[test]
fn a_cap_of_nothing_is_already_spent() {
    // A declared cap of zero is not "no cap"; a cap nobody declared is absent
    // altogether, and this is somebody who typed a zero.
    let cap = Cap {
        monthly: 0,
        exceeded: WhenExceeded::Pause,
    };
    assert_eq!(cap.reached(0), Reached::Exceeded);
}

#[test]
fn a_figure_cannot_be_counted_without_what_it_leaves_out() {
    // The only constructor attaches it, so a count of the stack's own traffic
    // cannot reach a surface looking like the household's total.
    let month = Metered::of("2026-09", 40, 2, Vec::new());
    assert_eq!(month.excludes, ONLY_THE_STACK);
    assert!(month.excludes.contains("phones"));
    assert!(month.excludes.contains("not counted here"));
    assert_eq!(month.moved(), 42);
}

#[test]
fn a_sum_that_would_wrap_stops_at_the_top_instead() {
    assert_eq!(
        Metered::of("2026-09", u64::MAX, 1, Vec::new()).moved(),
        u64::MAX
    );
}

#[test]
fn what_happens_at_the_cap_is_a_choice_with_words_for_it() {
    for choice in [
        WhenExceeded::Pause,
        WhenExceeded::Throttle,
        WhenExceeded::Continue,
    ] {
        let word = choice.word();
        assert_eq!(WhenExceeded::read(word), Some(choice), "{word}");
        assert_eq!(WhenExceeded::read(&word.to_uppercase()), Some(choice));
        assert!(!choice.means().is_empty(), "{word}");
    }
    assert_eq!(WhenExceeded::read("whatever"), None);
}

#[test]
fn only_a_month_that_is_going_wrong_is_worth_putting_in_front_of_anybody() {
    assert!(!Reached::Within.worth_saying());
    assert!(Reached::Warning.worth_saying());
    assert!(Reached::Exceeded.worth_saying());
}
