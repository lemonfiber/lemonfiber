use super::{for_the_house, NO_ROOM, RETURNS};
use crate::quality::{Preset, Selection};

/// A house with room and no limit says one thing, and it is what a thing costs.
#[test]
fn a_house_with_room_says_what_a_thing_costs() {
    let showing = for_the_house(&Selection::everywhere(Preset::Balanced), false, false);

    assert_eq!(
        showing.len(),
        1,
        "a house with room on the disk showed something other than the one notice \
         it has to show: {showing:?}"
    );
    assert!(
        showing
            .first()
            .is_some_and(|first| first.contains("A film") && first.contains("a season")),
        "the notice about what things cost named neither kind: {showing:?}"
    );
}

/// A house with no room says the disk as well, and says it is not a limit.
#[test]
fn a_house_with_no_room_says_the_disk_and_rules_out_the_limit() {
    let showing = for_the_house(&Selection::everywhere(Preset::Balanced), true, false);

    assert_eq!(showing.len(), 2, "the disk was not said: {showing:?}");
    assert!(
        showing.contains(&NO_ROOM.to_owned()),
        "a house with no room did not say so: {showing:?}"
    );
    assert!(
        NO_ROOM.contains("not your limit"),
        "the disk's own answer did not rule out the limit, which is the one move a \
         member would otherwise make"
    );
}

/// A house where anything is counted over a period is told how the period behaves.
///
/// The half the request service has nowhere to say. Its own sentence gives the length
/// and reads as a day of the week, so somebody who has run out waits for a reset that
/// never comes; this says room arrives a request at a time instead.
#[test]
fn a_house_under_a_limit_is_told_the_period_returns_room_a_request_at_a_time() {
    let showing = for_the_house(&Selection::everywhere(Preset::Balanced), false, true);

    assert_eq!(showing.len(), 2, "the period was not said: {showing:?}");
    assert!(
        showing.contains(&RETURNS.to_owned()),
        "a house under a limit was not told how the period frees up: {showing:?}"
    );
    assert!(
        RETURNS.contains("one at a time"),
        "the period's shape was said without the half that stops the waiting"
    );
}

/// And a house nothing is counted over is not told about a period it is not under.
#[test]
fn a_house_under_nothing_is_not_told_about_a_period() {
    let showing = for_the_house(&Selection::everywhere(Preset::Balanced), false, false);

    assert!(
        !showing.contains(&RETURNS.to_owned()),
        "a house holding nobody to a limit was told how one frees up: {showing:?}"
    );
}

/// A full disk takes the period's place rather than standing beside it.
///
/// The two together read as the disk being somebody's allowance, which is the one
/// reading the disk's own sentence exists to deny.
#[test]
fn a_full_disk_takes_the_periods_place_rather_than_standing_beside_it() {
    let showing = for_the_house(&Selection::everywhere(Preset::Balanced), true, true);

    assert_eq!(showing.len(), 2, "both were shown at once: {showing:?}");
    assert!(
        showing.contains(&NO_ROOM.to_owned()) && !showing.contains(&RETURNS.to_owned()),
        "a full disk was shown beside how a limit frees up: {showing:?}"
    );
}

/// The period's shape names no figure, because the figures are per person.
///
/// A house whose default is a week carries members held to a month, so a number on a
/// line everybody reads would be the wrong number for exactly those people.
#[test]
fn the_periods_shape_names_no_figure() {
    assert!(
        !RETURNS.chars().any(|letter| letter.is_ascii_digit()),
        "a line the whole house reads named one person's figure: {RETURNS}"
    );
}

/// Every notice fits the one line the request service clips it to.
///
/// Held at the width a wide window reads whole rather than at the field's own limit,
/// which is none: a sentence the service stores in full and shows a third of is one
/// nobody has been told. The figure is measured in a browser against the pinned
/// image, so a preset added above the current top one cannot quietly overrun it.
#[test]
fn every_notice_fits_the_line_it_is_shown_on() {
    /// What a heading holds before it is cut at a wide window.
    const READABLE: usize = 70;

    for preset in Preset::ALL {
        for no_room in [false, true] {
            for limited in [false, true] {
                for notice in for_the_house(&Selection::everywhere(preset), no_room, limited) {
                    assert!(
                        notice.chars().count() <= READABLE,
                        "a notice past {READABLE} characters is one the household \
                         reads the front of: {notice}"
                    );
                }
            }
        }
    }
}

/// What each notice is *for* survives the width a telephone cuts it to.
///
/// The cap above says a notice is readable whole somewhere. This says it is worth
/// reading where it is not: a heading cut to twenty-eight characters must already
/// have said the thing it exists to say, or the household is shown a preamble.
#[test]
fn what_each_notice_is_for_survives_a_telephone() {
    /// What a heading holds at the width of a telephone, measured in a browser.
    const NARROW: usize = 28;

    /// The first `NARROW` characters of each notice, as a telephone shows them.
    fn read(showing: &[String], narrow: usize) -> Vec<String> {
        showing
            .iter()
            .map(|notice| notice.chars().take(narrow).collect())
            .collect()
    }

    for preset in Preset::ALL {
        let disk = read(
            &for_the_house(&Selection::everywhere(preset), true, false),
            NARROW,
        );
        assert!(
            disk.first()
                .is_some_and(|costs| costs.chars().any(|letter| letter.is_ascii_digit())),
            "the notice about what things cost names no figure in the {NARROW} \
             characters a telephone shows: {disk:?}"
        );
        assert!(
            disk.get(1).is_some_and(|said| said.contains("disk")),
            "the notice about the disk does not name the disk in the {NARROW} \
             characters a telephone shows: {disk:?}"
        );

        let period = read(
            &for_the_house(&Selection::everywhere(preset), false, true),
            NARROW,
        );
        assert!(
            period
                .get(1)
                .is_some_and(|said| said.contains("Room returns one at a time")),
            "the notice about the period does not say room comes back a request at \
             a time in the {NARROW} characters a telephone shows: {period:?}"
        );
    }
}

/// The two kinds are told apart, so a bigger preset reads as bigger.
#[test]
fn a_costlier_quality_reads_as_costlier() {
    let thrifty = for_the_house(&Selection::everywhere(Preset::SpaceSaving), false, false).pop();
    let lavish = for_the_house(&Selection::everywhere(Preset::Maximum), false, false).pop();

    assert!(thrifty.is_some() && lavish.is_some(), "both were quoted");
    assert_ne!(
        thrifty, lavish,
        "two qualities a household would choose between were quoted the same price"
    );
}
