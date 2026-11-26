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
    if let Some(address) = &report.address {
        qr::labelled(
            &mut lines,
            "Or point a phone's camera at this:",
            &address.url,
        );
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

/// The front door's address as a code a camera reads, under what it is and over the
/// address it was drawn from, for a box that shows it full size.
pub(crate) fn code(address: &str) -> Lines {
    let mut lines = Lines::default();
    lines.put("Point a phone's camera at this to open the front door:".to_owned());
    qr::drawn(&mut lines, address);
    lines.put(format!("  {address}"));
    lines
}

#[cfg(test)]
mod tests;
