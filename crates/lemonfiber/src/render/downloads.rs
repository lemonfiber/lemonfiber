//! What is still coming down, said in a way an operator can act on.

use lemonfiber_core::app::Interrupted;
use lemonfiber_core::dashboard::Protocol;
use lemonfiber_core::plural::s;
use lemonfiber_core::text::fitted;

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
pub(crate) fn interrupting(active: &[Interrupted]) -> Lines {
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

#[cfg(test)]
mod tests;
