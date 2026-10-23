use super::{for_the_house, NO_ROOM, RETURNS};

/// Words no member is shown: a size, a service, the machine and how it is run.
pub(in crate::household::run) const NOT_FOR_THE_HOUSEHOLD: [&str; 12] = [
    "GB",
    "MB",
    "disk",
    "server",
    "service",
    "lemonfiber",
    "Seerr",
    "Jellyfin",
    "stack",
    "quality",
    "gigabyte",
    "storage",
];

/// Every notice the house could be shown.
fn every_notice() -> Vec<String> {
    [false, true]
        .into_iter()
        .flat_map(|no_room| [false, true].map(|limited| (no_room, limited)))
        .flat_map(|(no_room, limited)| for_the_house(no_room, limited))
        .collect()
}

/// A house with room and no limit has nothing to say to everybody.
#[test]
fn a_house_with_room_and_no_limit_says_nothing() {
    assert!(for_the_house(false, false).is_empty());
}

/// A house with no room says so as the house's, and says it is not a limit.
#[test]
fn a_house_with_no_room_says_so_and_rules_out_the_limit() {
    assert_eq!(for_the_house(true, false), vec![NO_ROOM.to_owned()]);
    assert!(
        NO_ROOM.contains("not your limit"),
        "the house's own answer did not rule out the limit, which is the one move a \
         member would otherwise make"
    );
    assert!(NO_ROOM.starts_with("No room for new things right now"));
}

/// A house where anything is counted over a period is told how the period behaves.
///
/// The half the request service has nowhere to say. Its own sentence gives the length
/// and reads as a day of the week, so somebody who has run out waits for a reset that
/// never comes; this says room arrives a request at a time instead.
#[test]
fn a_house_under_a_limit_is_told_the_period_returns_room_a_request_at_a_time() {
    assert_eq!(for_the_house(false, true), vec![RETURNS.to_owned()]);
    assert!(
        RETURNS.contains("one at a time"),
        "the period's shape was said without the half that stops the waiting"
    );
}

/// No room takes the period's place rather than standing beside it.
///
/// The two together read as a full house being somebody's allowance, which is the one
/// reading the house's own sentence exists to deny.
#[test]
fn no_room_takes_the_periods_place_rather_than_standing_beside_it() {
    assert_eq!(for_the_house(true, true), vec![NO_ROOM.to_owned()]);
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
/// nobody has been told. The figure is measured in a browser against the pinned image.
#[test]
fn every_notice_fits_the_line_it_is_shown_on() {
    /// What a heading holds before it is cut at a wide window.
    const READABLE: usize = 70;

    for notice in every_notice() {
        assert!(
            notice.chars().count() <= READABLE,
            "a notice past {READABLE} characters is one the household reads the front \
             of: {notice}"
        );
    }
}

/// What each notice is for survives the width a telephone cuts it to.
///
/// A heading cut to twenty-eight characters must already have said the thing it exists
/// to say, or the household is shown a preamble.
#[test]
fn what_each_notice_is_for_survives_a_telephone() {
    /// What a heading holds at the width of a telephone, measured in a browser.
    const NARROW: usize = 28;

    let narrow = |notice: &str| notice.chars().take(NARROW).collect::<String>();
    assert!(narrow(NO_ROOM).contains("No room for new things"));
    assert!(narrow(RETURNS).contains("Room returns one at a time"));
}

/// Nothing the house is shown names a size, a service or how the house is run.
#[test]
fn nothing_the_house_is_shown_is_in_anything_but_its_own_words() {
    for notice in every_notice() {
        for word in NOT_FOR_THE_HOUSEHOLD {
            assert!(
                !notice.contains(word),
                "the household was shown {word:?}: {notice}"
            );
        }
    }
}
