//! What is still coming down, said in a way an operator can act on.

use lemonfiber_core::app::{InFlightReport, Unfinished};
use lemonfiber_core::dashboard::Protocol;
use lemonfiber_error::plural::s;
use lemonfiber_words::text::fitted;

use super::Lines;

/// How wide the name column is before a name is shortened.
const NAMED: usize = 40;

/// The word for a protocol.
///
/// Here rather than beside the screen that first needed it, because the word a
/// download is described by should not depend on which surface is describing it.
pub(crate) const fn protocol(protocol: Protocol) -> &'static str {
    match protocol {
        Protocol::Usenet => "usenet",
        Protocol::Torrent => "torrent",
    }
}

/// What stopping would interrupt, named one by one.
///
/// Named rather than counted. "3 downloads still active" and a list naming them lead
/// to different decisions, and the question an operator actually has is whether the
/// one thing they have been waiting for is among them — which a number cannot answer.
///
/// The client is named beside each, because "still downloading" is only half of what
/// they need: the other half is which of the two clients to go and look in.
pub(crate) fn interrupting(active: &[Unfinished]) -> Lines {
    let mut lines = Lines::default();
    lines.spaced(format!(
        "{} download{} still active:",
        active.len(),
        s(active.len())
    ));
    for download in active {
        lines.put(format!(
            "  {:<8}  {:<width$}  {:>3}%",
            protocol(download.protocol),
            fitted(&download.name, NAMED),
            download.progress,
            width = NAMED
        ));
    }
    lines
}

/// What is coming down, or that nothing is.
pub(super) fn in_flight(report: &InFlightReport) -> Lines {
    if report.downloads.is_empty() {
        let mut lines = Lines::default();
        lines.put("Nothing is coming down.");
        return lines;
    }
    interrupting(&report.downloads)
}

#[cfg(test)]
mod tests;
