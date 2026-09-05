//! Roughly what one request will cost the disk, said as a guess and never as a figure.
//!
//! **The request service has no notion of size at all.** Nothing in its records, its
//! settings or its API carries bytes — checked against `ghcr.io/seerr-team/seerr:v3.3.0`
//! rather than recalled — so a household member is shown no cost as they ask, and
//! nothing there can be made to show them one. What can be shown is this: what a thing
//! is likely to take, worked out from the quality in force and how long a thing of that
//! kind usually runs, put in front of whoever is about to say yes to it.
//!
//! **The number is wrong and says so.** An hour of television is not a fixed size, a
//! season is not a fixed number of episodes, and a film is not a fixed length. What the
//! figure is good for is the difference between forty gigabytes and four hundred, which
//! is the difference that changes anybody's mind — and a number offered without the word
//! "about" in front of it is a number somebody will hold this product to.
//!
//! The per-hour figure is the quality module's own, not a second table beside it. Two
//! tables of sizes would disagree the first time a preset was retuned, and the place
//! they would disagree is a projection of the disk against an estimate of what is about
//! to land on it.

use crate::quality::Preset;

/// Roughly how long a film runs, in hours.
///
/// A representative feature rather than a longest one: the estimate exists to catch
/// somebody asking for far more than they realise, and an upper bound applied to every
/// request would overstate every one of them by the same factor.
pub const FILM_HOURS: u64 = 2;

/// Roughly how long a season of television runs, in hours.
///
/// Ten episodes of about forty-five minutes, which is the shape of most of what a
/// household asks for. A twenty-two-episode network season is half again as much and a
/// six-part drama is half as much, which is the width of the guess and why it is called
/// one.
pub const SEASON_HOURS: u64 = 8;

/// About how much room one request will want.
///
/// Carried as a number and a word rather than as a rendered string, so a surface can
/// put it in a column and still say what it is. The word is not decoration: a figure
/// with nothing hedging it is a promise, and this cannot keep one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, schemars::JsonSchema)]
pub struct Estimate {
    /// About how many bytes it will take.
    pub bytes: u64,
    /// Whether this was measured. Always false — nothing here has measured anything,
    /// and the field is present so that a surface rendering it cannot forget to say so.
    pub measured: bool,
}

impl Estimate {
    /// About what one film at this quality will take.
    #[must_use]
    pub const fn film(preset: Preset) -> Self {
        Self::running(preset, FILM_HOURS)
    }

    /// About what one season of television at this quality will take.
    #[must_use]
    pub const fn season(preset: Preset) -> Self {
        Self::running(preset, SEASON_HOURS)
    }

    /// About what content of this length at this quality will take.
    const fn running(preset: Preset, hours: u64) -> Self {
        Self {
            bytes: preset.bytes_per_hour().saturating_mul(hours),
            measured: false,
        }
    }

    /// How it reads, with the hedge in front of it.
    ///
    /// The word is part of the value rather than something a surface adds, because
    /// three surfaces adding it separately is two of them eventually not.
    #[must_use]
    pub fn reading(self) -> String {
        format!("about {}", crate::bytes::humanize(self.bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::Estimate;
    use crate::quality::Preset;

    /// A season is more than a film at the same quality, which is the whole of what
    /// the figure is for.
    #[test]
    fn a_season_costs_more_than_a_film_at_the_same_quality() {
        assert!(Estimate::season(Preset::Balanced).bytes > Estimate::film(Preset::Balanced).bytes);
    }

    /// A higher preset costs more than a lower one for the same thing.
    #[test]
    fn a_higher_quality_costs_more_for_the_same_thing() {
        let mut last = 0;
        for preset in Preset::ALL {
            let estimated = Estimate::film(preset).bytes;
            assert!(
                estimated > last,
                "{preset:?} is not dearer than the one below"
            );
            last = estimated;
        }
    }

    /// The figure carries the hedge, so a surface cannot render it as a measurement.
    #[test]
    fn the_figure_carries_the_word_that_makes_it_a_guess() {
        let estimated = Estimate::season(Preset::Maximum);

        assert!(!estimated.measured, "nothing here has measured anything");
        assert!(estimated.reading().starts_with("about "), "{estimated:?}");
        assert!(estimated.reading().contains("GiB"), "{estimated:?}");
    }

    /// It serialises with the word beside the number, which is what a browser reads.
    #[test]
    fn it_serialises_with_the_word_beside_the_number() {
        let written =
            serde_json::to_string(&Estimate::film(Preset::SpaceSaving)).unwrap_or_default();

        assert!(written.contains("\"measured\":false"), "{written}");
        assert!(written.contains("\"bytes\":1500000000"), "{written}");
    }
}
