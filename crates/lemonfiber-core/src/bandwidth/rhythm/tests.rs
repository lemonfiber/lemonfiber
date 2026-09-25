use super::{Period, Rhythm, Wall, DAY};

/// The window every case here reads against: awake from seven to eleven.
fn a_day() -> Rhythm {
    Rhythm {
        from: Wall { hour: 7, minute: 0 },
        to: Wall {
            hour: 23,
            minute: 0,
        },
    }
}

/// A wall-clock time from minutes into the day.
fn at(minutes: u16) -> Wall {
    Wall {
        hour: u8::try_from(minutes / 60).unwrap_or(0),
        minute: u8::try_from(minutes % 60).unwrap_or(0),
    }
}

/// A window this file's cases read against, or a day this file already knows,
/// so a case whose window would not parse fails on its own assertion rather
/// than quietly reading the wrong window.
fn window(text: &str) -> Rhythm {
    let read = Rhythm::read(text);
    assert!(read.is_some(), "{text} did not read as a window");
    read.unwrap_or_else(a_day)
}

#[test]
fn a_window_is_read_the_way_a_household_would_write_one() {
    let read = Rhythm::read("07:00-23:00");
    assert_eq!(read, Some(a_day()));
    assert_eq!(
        read.map(|window| window.says()),
        Some("07:00 to 23:00".to_owned())
    );
    assert_eq!(a_day().active_minutes(), 16 * 60);
    assert!(!a_day().wraps());
}

#[test]
fn a_time_in_a_frame_this_cannot_place_is_not_read_at_all() {
    // The whole of what makes a daylight-saving transition a non-event. A
    // window stored against an offset moves an hour twice a year, and the
    // boundary is then either skipped or applied twice — so the offset is
    // refused on the way in rather than dropped on the way through.
    assert_eq!(Wall::read("07:00Z"), None);
    assert_eq!(Wall::read("07:00+02:00"), None);
    assert_eq!(Rhythm::read("07:00+02:00-23:00"), None);
    assert_eq!(Wall::read("7:00"), None, "the shape is fixed-width");
    assert_eq!(Wall::read("24:00"), None);
    assert_eq!(Wall::read("07:60"), None);
    assert_eq!(Wall::read("0700"), None);
    assert_eq!(Wall::read("aa:bb"), None);
    assert_eq!(Rhythm::read("07:00"), None);
    assert_eq!(Rhythm::read("07:00-oops"), None);
    assert_eq!(Rhythm::read("oops-07:00"), None);
}

#[test]
fn a_window_that_starts_where_it_ends_is_refused_rather_than_resolved() {
    // It could as easily mean the whole day as none of it, and a household
    // would find out which by living through an evening.
    assert_eq!(Rhythm::read("07:00-07:00"), None);
}

#[test]
fn the_minutes_the_window_claims_are_the_minutes_it_is_long() {
    // Two independent readings of the same window: the arithmetic that says
    // how long the active hours run, and the classification a client would
    // make minute by minute. They agree or the boundary is applied twice at
    // one end and skipped at the other — the window is closed where it starts
    // and open where it ends, so the two halves of the day meet exactly once.
    for window in [a_day(), window("23:00-07:00"), window("00:00-00:01")] {
        let active = (0..DAY)
            .filter(|minute| window.holds(at(*minute)) == Period::Active)
            .count();
        assert_eq!(active, usize::from(window.active_minutes()), "{window:?}");
        assert!(active > 0 && active < usize::from(DAY), "{window:?}");
    }
}

#[test]
fn a_household_that_is_up_late_gets_a_window_that_runs_past_midnight() {
    let late = window("18:00-02:00");
    assert!(late.wraps());
    assert_eq!(late.active_minutes(), 8 * 60);
    assert_eq!(late.holds(at(19 * 60)), Period::Active);
    assert_eq!(late.holds(at(60)), Period::Active, "after midnight");
    assert_eq!(late.holds(at(3 * 60)), Period::Quiet);
    assert_eq!(late.holds(at(2 * 60)), Period::Quiet, "the end is open");
    assert_eq!(
        late.holds(at(18 * 60)),
        Period::Active,
        "the start is closed"
    );
}

#[test]
fn a_period_says_what_it_means_for_the_house() {
    assert!(Period::Active.means().contains("held to its limits"));
    assert!(Period::Quiet.means().contains("has the line"));
}

#[test]
fn a_window_survives_the_record_it_is_kept_in() {
    let written = serde_json::to_string(&a_day()).unwrap_or_default();
    assert_eq!(written, r#"{"from":"07:00","to":"23:00"}"#);
    assert_eq!(serde_json::from_str::<Rhythm>(&written).ok(), Some(a_day()));
    assert!(serde_json::from_str::<Rhythm>(r#"{"from":"7am","to":"23:00"}"#).is_err());
}

#[test]
fn the_parts_a_clients_own_scheduler_asks_for_come_off_the_wall_clock() {
    assert_eq!(a_day().from.hour(), 7);
    assert_eq!(a_day().to.minute(), 0);
    assert_eq!(a_day().from.to_string(), "07:00");
}
