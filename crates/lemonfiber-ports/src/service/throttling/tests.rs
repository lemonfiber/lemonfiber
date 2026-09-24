use super::{Hours, Rates, Throttled, Wanted, Window};

#[test]
fn nothing_held_back_is_absence_rather_than_a_zero() {
    // A client asked for a limit of zero is a client asked to move nothing,
    // which is what a torrent client's own API means by it. No limit at all
    // has to be a different value or the two are one setting.
    assert_eq!(
        Rates::default(),
        Rates {
            down: None,
            up: None
        }
    );
    assert_ne!(
        Rates::default(),
        Rates {
            down: Some(0),
            up: Some(0)
        }
    );
}

#[test]
fn a_client_with_no_scheduler_is_told_apart_from_one_inside_its_hours() {
    let keeping = Throttled {
        rates: Rates::default(),
        uploads: true,
        hours: Some(Hours::Active),
    };
    let keeping_none = Throttled {
        hours: None,
        ..keeping
    };
    assert_ne!(keeping, keeping_none);
    assert_ne!(
        keeping_none,
        Throttled {
            hours: Some(Hours::Quiet),
            ..keeping
        }
    );
}

#[test]
fn a_restraint_with_no_window_is_the_active_rates_around_the_clock() {
    // The conservative direction, and the only honest one for a client with
    // no scheduler: the household is protected all day rather than at no
    // point in it.
    let held = Wanted {
        active: Rates {
            down: Some(1_000),
            up: Some(100),
        },
        quiet: Rates::default(),
        window: None,
    };
    assert!(held.window.is_none());
    assert_eq!(held.active.down, Some(1_000));
    assert_ne!(
        held,
        Wanted {
            window: Some(Window {
                from_hour: 7,
                from_minute: 0,
                to_hour: 23,
                to_minute: 0,
            }),
            ..held
        }
    );
}
