//! The one address to hand somebody who lives here.
//!
//! One of the renderers, its own file so each answer's shape is read on its own.
//! Every one of them builds lines and hands them back; the printer is at the edge.
//!
//! The shape is the answer first and the working underneath it. An operator who
//! only wanted to know what to send has it in the first two lines — what it is
//! called and the address itself — and one who wondered why it was not the page
//! that links everything reads on and finds that question answered by name.

use lemonfiber_core::model::{FrontDoorReport, Standing};

use super::{qr, Lines};
use crate::say;

/// What the household begins at, and what stands beside it that is not it.
pub(super) fn front_door(report: &FrontDoorReport) -> Lines {
    let mut lines = Lines::default();
    match &report.service {
        Some(service) => lines.put(format!("{service}   {}", standing(report.standing))),
        None => lines.put(standing(report.standing)),
    }
    if let Some(address) = &report.address {
        lines.put(format!("  {}", address.url));
    }
    lines.put(format!("  {}", report.meaning));
    if let Some(caution) = report
        .address
        .as_ref()
        .and_then(|address| address.caution.as_ref())
    {
        lines.put(format!("  {caution}"));
    }

    if !report.beside.is_empty() {
        lines.spaced("Also on your network, and none of them a way in:");
        for beside in &report.beside {
            lines.put(format!("  {}   {}", beside.service, beside.because));
        }
    }

    // Last, under everything. An operator who wanted the address to send has it in
    // the second line, and a picture put between that and the working would push
    // the working off the screen for the reader who wanted that instead.
    if let Some(drawn) = report
        .address
        .as_ref()
        .and_then(|address| qr::rows(&address.url, say::folding()))
    {
        lines.spaced("Or point a phone's camera at this:");
        for row in drawn {
            lines.put(format!("  {row}"));
        }
    }
    lines
}

/// Where the door stands, in one phrase.
///
/// Shared with the dashboard's own panel, which has room for a phrase and none for
/// the answer's sentence: two renderings of one screenful is two things to keep in
/// step, and the one nobody is looking at is the one that stops being true.
pub(crate) const fn standing(standing: Standing) -> &'static str {
    match standing {
        Standing::Established => "the front door",
        Standing::LibraryOnly => "the front door, and there is nothing here to ask for",
        Standing::Unreachable => "the front door, and it is not answering",
        Standing::Stranded => "the front door, and there is no address to arrive at",
        Standing::Absent => "There is no front door.",
    }
}

#[cfg(test)]
mod tests;
