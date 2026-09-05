//! What the whole house is told, put where the house will read it.
//!
//! [`handing_over`](super::handing_over) writes to one member and hands the message to
//! the operator, because most of what a member is owed is about *them* — what their
//! period has left, what they are waiting on, why something of theirs was refused — and
//! there is one page where they ask, seen by everybody. Two of those sentences are not
//! about anybody in particular: what a thing costs at the quality this house is set to,
//! and whether the disk has room. Those two are true of the house, so those two can go on
//! the one page.
//!
//! **The figures are the same figures.** Both this and the message handed over read
//! [`allowance::for_kind`](super::allowance::for_kind), because what somebody is told a
//! film costs before they ask and what they are told beside a request they made are the
//! same claim, and a second table would be the first place those two disagreed.
//!
//! **The wording is not the same wording, and that is the medium rather than a second
//! opinion.** The request service shows a notice as a heading on one line it clips rather
//! than wraps: about seventy characters at a wide window and **twenty-eight** at the
//! width of a telephone, measured in a browser against `ghcr.io/seerr-team/seerr:v3.3.0`
//! rather than guessed. So a notice leads with the fact that changes a decision and lets
//! the qualification be the half that is lost — twenty-eight characters is enough for
//! what a film costs, or for there being no room on the disk, and that is what each of
//! them is for. The message handed to the operator has no width to fit and says the whole
//! thing.

use crate::ports::service::Noticing;
use crate::quality::Selection;
use crate::recyclarr::Kind;

/// What the house is showing everybody who is about to ask for something.
///
/// Ordered as [`handing_over`](super::handing_over) orders the same two facts. Two
/// headings sit next to each other at the top of one page, so which comes first is not
/// worth a second answer to what order these go in.
#[must_use]
pub(super) fn for_the_house(quality: &Selection, no_room: bool) -> Vec<String> {
    let mut showing = vec![costs(quality)];
    if no_room {
        showing.push(NO_ROOM.to_owned());
    }
    showing
}

/// What the disk says, in the disk's own words and never as somebody's limit.
///
/// The limit is named only to be ruled out: waiting for a period to roll over is the one
/// move a member would otherwise make, and it is the one that cannot help.
const NO_ROOM: &str = "No room on the disk — not your limit, so nothing new is fetched";

/// Roughly what the two kinds of thing take, before anybody has chosen one.
///
/// The figures lead because the line is cut from the right at twenty-eight characters on
/// a telephone: a member who reads only as far as what a film costs has still been told
/// something they did not know, and one who reads none of the hedging that follows has
/// not been told anything false, because the hedging is inside the figure.
fn costs(quality: &Selection) -> String {
    format!(
        "A film {}, a season {} — before you ask",
        super::allowance::for_kind(Kind::Radarr, quality).reading(),
        super::allowance::for_kind(Kind::Sonarr, quality).reading(),
    )
}

/// Put the house's notices in front of the household, and say so where it did not work.
///
/// Answers with what went wrong rather than failing: this rides along on a reading, and a
/// household list that refused to be read because a notice could not be hung would report
/// nothing about anybody over something nobody asked for.
pub(super) async fn put_where_they_ask(
    seerr: &dyn Noticing,
    quality: &Selection,
    no_room: bool,
    dry_run: bool,
) -> Option<String> {
    if dry_run {
        return None;
    }
    seerr
        .set_notices(&for_the_house(quality, no_room))
        .await
        .err()
        .map(|_| {
            "the request service would not carry what the household is told before they \
             ask, so what a thing costs and whether the disk has room are shown here and \
             not where they ask"
                .to_owned()
        })
}

#[cfg(test)]
mod tests {
    use super::{for_the_house, NO_ROOM};
    use crate::quality::{Preset, Selection};

    /// A house with room says one thing, and it is what a thing costs.
    #[test]
    fn a_house_with_room_says_what_a_thing_costs() {
        let showing = for_the_house(&Selection::everywhere(Preset::Balanced), false);

        assert_eq!(
            showing.len(),
            1,
            "a house with room on the disk showed something other than the one notice \
             it has to show: {showing:?}"
        );
        let Some(first) = showing.first() else {
            unreachable!("a list of one has a first");
        };
        assert!(
            first.contains("A film") && first.contains("a season"),
            "the notice about what things cost named neither kind: {first}"
        );
    }

    /// A house with no room says the disk as well, and says it is not a limit.
    #[test]
    fn a_house_with_no_room_says_the_disk_and_rules_out_the_limit() {
        let showing = for_the_house(&Selection::everywhere(Preset::Balanced), true);

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
                for notice in for_the_house(&Selection::everywhere(preset), no_room) {
                    assert!(
                        notice.chars().count() <= READABLE,
                        "a notice past {READABLE} characters is one the household reads \
                         the front of: {notice}"
                    );
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

        for preset in Preset::ALL {
            let showing = for_the_house(&Selection::everywhere(preset), true);
            let read: Vec<String> = showing
                .iter()
                .map(|notice| notice.chars().take(NARROW).collect())
                .collect();
            let Some(costs) = read.first() else {
                unreachable!("a house with no room shows two notices");
            };
            assert!(
                costs.chars().any(|letter| letter.is_ascii_digit()),
                "the notice about what things cost names no figure in the {NARROW} \
                 characters a telephone shows: {costs}"
            );
            let Some(disk) = read.get(1) else {
                unreachable!("a house with no room shows two notices");
            };
            assert!(
                disk.contains("disk"),
                "the notice about the disk does not name the disk in the {NARROW} \
                 characters a telephone shows: {disk}"
            );
        }
    }

    /// The two kinds are told apart, so a bigger preset reads as bigger.
    #[test]
    fn a_costlier_quality_reads_as_costlier() {
        let Some(thrifty) = for_the_house(&Selection::everywhere(Preset::SpaceSaving), false).pop()
        else {
            unreachable!("a list of one has a last");
        };
        let Some(lavish) = for_the_house(&Selection::everywhere(Preset::Maximum), false).pop()
        else {
            unreachable!("a list of one has a last");
        };

        assert_ne!(
            thrifty, lavish,
            "two qualities a household would choose between were quoted the same price"
        );
    }
}
