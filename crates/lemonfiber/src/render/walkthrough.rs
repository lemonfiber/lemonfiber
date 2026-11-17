//! Showing a walkthrough — whether one is worth taking, as it happens, and once it has.
//!
//! The two halves are genuinely different jobs. While it runs, each line has to appear the
//! moment it is true, on a terminal, one at a time; once it is over, the whole thing is a
//! report with an ending, a diagnosis or a handover, and — under `--json` — one document.
//! Neither is a special case of the other, so they are a file each.

mod live;
mod report;

use lemonfiber_core::walkthrough::Why;

use super::Lines;

pub(crate) use live::{is_worth_saying, spoken, Narrating, Quiet};
pub(crate) use report::ending;

/// Whether a walkthrough is worth taking here, and what it would prove or why not.
pub(super) fn offer(why: Why) -> Lines {
    let mut lines = Lines::default();
    match why {
        Why::Offer(shape) => {
            lines.put(format!("A walkthrough would prove {}.", shape.proves()));
        }
        Why::Not(reason) => {
            lines.put(reason.said());
            lines.put(format!("  → {}", reason.remedy()));
        }
    }
    lines
}
