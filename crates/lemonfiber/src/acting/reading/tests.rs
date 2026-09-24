use super::Reading;

/// A reading over nine numbered lines.
fn nine() -> Reading {
    Reading::of((0..9).map(|at| format!("line {at}")).collect())
}

/// What the box shows, as one piece of text.
fn shown(reading: &Reading, rows: usize) -> String {
    reading.window(rows).0.join("\n")
}

/// A reading opens at the first line and says how much is under it.
#[test]
fn a_reading_opens_at_the_beginning() {
    let reading = nine();

    let (shown, above, below) = reading.window(4);

    assert_eq!(shown, vec!["line 0", "line 1", "line 2", "line 3"]);
    assert_eq!((above, below), (0, 5));
}

/// Moving down and back up again lands where it started, which is what makes
/// scrolling past something recoverable rather than a reason to ask again.
#[test]
fn moving_through_it_and_back_lands_where_it_started() {
    let mut reading = nine();

    reading.forward();
    reading.forward();
    assert_eq!(shown(&reading, 2), "line 2\nline 3");
    assert_eq!(reading.window(2), (vec!["line 2", "line 3"], 2, 5));

    reading.back();
    reading.back();
    assert_eq!(shown(&reading, 2), "line 0\nline 1");
}

/// The ends hold. A box that scrolled off its own end would be empty over a
/// screen with plenty to say, and would read as the answer having been nothing.
#[test]
fn the_ends_of_a_reading_hold() {
    let mut reading = nine();

    reading.back();
    assert_eq!(shown(&reading, 1), "line 0");

    for _ in 0..20 {
        reading.forward();
    }
    assert_eq!(shown(&reading, 3), "line 8");
    assert_eq!(reading.window(3), (vec!["line 8"], 8, 0));
}

/// A reading with nothing in it is a box that says nothing rather than a move
/// that goes wrong — a command that answered with no lines at all is rare and
/// is not a fault in this.
#[test]
fn a_reading_over_nothing_moves_nowhere() {
    let mut reading = Reading::of(Vec::new());

    reading.forward();
    reading.back();

    assert_eq!(reading.window(5), (Vec::new(), 0, 0));
}

/// A box with no room shows nothing and still counts what it has not shown, so
/// the count under it is never a claim that there is nothing.
#[test]
fn a_box_with_no_room_still_counts_what_it_holds() {
    assert_eq!(nine().window(0), (Vec::new(), 0, 9));
}
