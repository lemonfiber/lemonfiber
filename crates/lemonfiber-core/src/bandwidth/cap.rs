//! A monthly data cap, and what the stack has spent against it.
//!
//! On a metered connection uncontrolled seeding is genuinely expensive, and it is
//! the usual culprit: it continues indefinitely and produces nothing visible, so
//! the month it ate is discovered on a bill.
//!
//! Two rules shape everything here.
//!
//! **The count is the stack's own, and says so.** lemonfiber can measure what its
//! download clients moved. It cannot measure the phone in the next room, the
//! console, or the work laptop, and it must not let a figure it produced be read as
//! the household's total — a cap that reads as three quarters spent when the meter
//! at the provider says ninety-five per cent is worse than no figure, because it is
//! believed.
//!
//! **What happens at the cap is decided in advance.** Not prompted at the moment,
//! which arrives at two in the morning on a stack nobody is watching. The choice is
//! made when the cap is declared, in the calm, and applied when it is reached.

use serde::{Deserialize, Serialize};

/// The share of a cap at which the warning starts, in whole per cent.
///
/// Late enough that a household on a generous plan is not warned every month, and
/// early enough that there is a tenth of the month's allowance left to change
/// something with.
pub const WARN_AT: u64 = 90;

/// The sentence that travels with every figure counted here.
///
/// Stated the same way wherever a cap figure appears, because the whole risk of
/// reporting one is that it gets read as the household's total.
pub const ONLY_THE_STACK: &str =
    "This counts what lemonfiber's own download clients moved. Everything else in \
     the house — phones, consoles, video calls, anybody streaming from outside — \
     is on the same line and is not counted here, so the meter your provider keeps \
     will read higher than this.";

/// What to do when a declared cap is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum WhenExceeded {
    /// Stop fetching until the month turns over.
    Pause,
    /// Keep going, slowly, so what is half-finished can finish.
    Throttle,
    /// Carry on. Some caps cost money and some only cost speed.
    Continue,
}

impl WhenExceeded {
    /// What this choice does, in the words it is chosen by.
    #[must_use]
    pub const fn means(self) -> &'static str {
        match self {
            Self::Pause => "nothing new is fetched until the month turns over",
            Self::Throttle => "fetching continues at a crawl, so what is half-finished can finish",
            Self::Continue => "nothing changes, because some caps cost speed rather than money",
        }
    }

    /// The choice as it is written and read back.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Pause => "pause",
            Self::Throttle => "throttle",
            Self::Continue => "continue",
        }
    }

    /// Read the choice from the word it is written as.
    #[must_use]
    pub fn read(text: &str) -> Option<Self> {
        [Self::Pause, Self::Throttle, Self::Continue]
            .into_iter()
            .find(|choice| choice.word().eq_ignore_ascii_case(text.trim()))
    }
}

/// A monthly allowance, and what to do at the end of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Cap {
    /// The allowance, in bytes.
    pub monthly: u64,
    /// What happens when it is reached, chosen when the cap was declared.
    pub exceeded: WhenExceeded,
}

/// Where a month stands against a declared cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Reached {
    /// Comfortably inside it.
    Within,
    /// Close enough that there is still time to do something.
    Warning,
    /// Reached, and the declared behaviour applies.
    Exceeded,
}

/// What the stack itself moved in a calendar month, and what that leaves out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Metered {
    /// The month, as the client that dated the figures dates them.
    pub month: String,
    /// Bytes pulled down, as far as the clients count them.
    pub down: u64,
    /// Bytes given back.
    pub up: u64,
    /// What this count does not include, always said.
    pub excludes: &'static str,
    /// What is known to be missing from the count itself, where anything is.
    pub incomplete: Vec<String>,
}

impl Metered {
    /// A month's figures, with the sentence that must travel with them.
    ///
    /// The only way to build one, so a figure counted here cannot reach a surface
    /// without what it leaves out attached — which is the whole of the risk in
    /// publishing it.
    #[must_use]
    pub fn of(month: impl Into<String>, down: u64, up: u64, incomplete: Vec<String>) -> Self {
        Self {
            month: month.into(),
            down,
            up,
            excludes: ONLY_THE_STACK,
            incomplete,
        }
    }

    /// Everything moved, both directions, which is what a cap is usually counted in.
    #[must_use]
    pub const fn moved(&self) -> u64 {
        self.down.saturating_add(self.up)
    }
}

impl Cap {
    /// Where a month's usage stands against this cap.
    #[must_use]
    pub const fn reached(&self, moved: u64) -> Reached {
        if self.monthly == 0 || moved >= self.monthly {
            return Reached::Exceeded;
        }
        // Scaled before dividing so a small cap does not round its own warning
        // away; a cap large enough to overflow this would be sixteen exabytes.
        if moved.saturating_mul(100) / self.monthly >= WARN_AT {
            return Reached::Warning;
        }
        Reached::Within
    }

    /// What is left of the allowance.
    #[must_use]
    pub const fn left(&self, moved: u64) -> u64 {
        self.monthly.saturating_sub(moved)
    }
}

impl Reached {
    /// Whether this is worth putting in front of the operator at all.
    #[must_use]
    pub const fn worth_saying(self) -> bool {
        !matches!(self, Self::Within)
    }
}

#[cfg(test)]
mod tests {
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
}
