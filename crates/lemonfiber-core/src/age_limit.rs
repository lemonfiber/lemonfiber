//! How far up an account may watch, in the one set of words every surface says it in.
//!
//! **The media server keeps this as a number, and the number is an age.** It holds back
//! everything it rates above that number. Driven against `jellyfin/jellyfin:10.10.3`:
//! its own rating tables, read at `GET /Localization/ParentalRatings`, put every
//! certificate against the age it is for — `TV-Y7` at 7, `12A` at 12, `PG-13` at 13,
//! `15` at 15, `18` at 18. So nought is not "nothing at all": it is everything the
//! youngest person in a house could watch, and it is said in words here for that
//! reason.
//!
//! **Certificates are country-specific.** The same tables read under a different
//! country give different names for the same numbers: `U` at nought and `PG` at eight
//! for the United Kingdom, `G` at nought and `PG` at ten for the United States. The age
//! is the same in both.
//!
//! **What the surfaces offer is a set of steps, not every value the server accepts.**
//! The server takes any number. A limit that is none of the steps is still said as the
//! age it is: an account may hold one set in the media server's own screens, or by
//! whoever ran this stack before.
//!
//! One place for the words, because they are said twice: when the limit is chosen, and
//! when it is read back off the account. Two copies would eventually disagree, and the
//! place they would disagree is a household list saying something other than what the
//! operator picked.

/// One age limit the surfaces offer, and who it suits.
pub struct Step {
    /// The age the media server keeps and holds things back above.
    pub age: u32,
    /// Who it suits, in one line — what choosing it comes to, beside the words for it.
    pub suits: &'static str,
}

/// The steps offered, lowest first.
///
/// Read in this order everywhere they are offered: a ladder from the youngest audience
/// upwards, which is how somebody deciding for a particular person in the house scans
/// it. No limit at all is not on this list, because it is the absence of a limit rather
/// than a step among them — a surface that offers it puts it where an answer that
/// changes nothing belongs, which on a command line is leaving the flag out.
const OFFERED: &[Step] = &[
    Step {
        age: 0,
        suits: "nothing the media server rates for an older audience",
    },
    Step {
        age: 7,
        suits: "about right for a young child",
    },
    Step {
        age: 12,
        suits: "about right for an older child",
    },
    Step {
        age: 15,
        suits: "about right for a teenager",
    },
    Step {
        age: 18,
        suits: "holds back only what is meant for adults",
    },
];

/// The steps offered, in the order they are read.
#[must_use]
pub fn steps() -> &'static [Step] {
    OFFERED
}

/// How an age limit reads, given the number the media server keeps it as.
///
/// Nought is said in words rather than as a figure, because "nothing above about 0" is
/// a sentence about a number and this is a sentence about a household. Every other age
/// is said as the age, whether or not it is one of the steps offered.
#[must_use]
pub fn reading(age: Option<u32>) -> String {
    match age {
        None => "anything".to_owned(),
        Some(0) => "only what suits everyone".to_owned(),
        Some(age) => format!("nothing above about {age}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{reading, steps};

    /// No limit, the youngest step and an ordinary age each read as their own words.
    #[test]
    fn each_limit_reads_as_the_words_for_it() {
        assert_eq!(reading(None), "anything");
        assert_eq!(reading(Some(0)), "only what suits everyone");
        assert_eq!(reading(Some(15)), "nothing above about 15");
    }

    /// A limit that is none of the steps offered still reads as something true.
    ///
    /// An account may carry one set in the media server's own screens, and a reader
    /// shown nothing would take that for an account with no limit on it at all.
    #[test]
    fn a_limit_that_is_not_a_step_offered_still_reads() {
        assert!(
            !steps().iter().any(|step| step.age == 13),
            "13 is a step offered, so this asserts nothing"
        );
        assert_eq!(reading(Some(13)), "nothing above about 13");
    }

    /// The steps rise, so a list of them reads as a ladder rather than as a set of
    /// unrelated numbers.
    #[test]
    fn the_steps_rise_from_the_youngest_audience() {
        let ages: Vec<u32> = steps().iter().map(|step| step.age).collect();

        assert!(ages.len() > 2, "there is no ladder to read: {ages:?}");
        assert_eq!(
            ages.first(),
            Some(&0),
            "the ladder does not start at nought"
        );
        assert!(ages.is_sorted(), "the steps do not rise: {ages:?}");
    }

    /// Every step says who it suits, or a list of them is a column of numbers with a
    /// blank beside each.
    #[test]
    fn every_step_says_who_it_suits() {
        for step in steps() {
            // Bound rather than called in the message, which only runs on failure.
            let said = reading(Some(step.age));
            assert!(!step.suits.is_empty(), "{said} has nothing beside it");
        }
    }
}
