//! Showing a walkthrough — as it happens, and once it has.
//!
//! The two halves are genuinely different jobs. While it runs, each line has to appear the
//! moment it is true, on a terminal, one at a time; once it is over, the whole thing is a
//! report with an ending, a diagnosis or a handover, and — under `--json` — one document.
//! Neither is a special case of the other, so they are a file each.

mod live;
mod report;

pub(crate) use live::{Narrating, Quiet};
pub(crate) use report::ending;
