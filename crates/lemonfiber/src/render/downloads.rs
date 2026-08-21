//! What is still coming down, said in a way an operator can act on.

use lemonfiber_core::app::Interrupted;
use lemonfiber_core::dashboard::Protocol;
use lemonfiber_core::plural::s;

use super::Lines;

/// How wide the name column is before a title is clipped.
///
/// Release names run long and a wrapped one is harder to recognise than a clipped
/// one — the front of a name is where the title is, and the tail is where the
/// encoding and the group are.
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
            "  {:<8}  {:<width$.width$}  {:>3}%",
            protocol(download.protocol),
            download.name,
            download.progress,
            width = NAMED
        ));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::{interrupting, protocol};
    use lemonfiber_core::app::Interrupted;
    use lemonfiber_core::dashboard::Protocol;

    /// One download, as the engine reports it.
    fn coming(protocol: Protocol, name: &str, progress: u8) -> Interrupted {
        Interrupted {
            protocol,
            name: name.to_owned(),
            progress,
        }
    }

    #[test]
    fn each_protocol_has_its_own_word() {
        assert_eq!(protocol(Protocol::Usenet), "usenet");
        assert_eq!(protocol(Protocol::Torrent), "torrent");
    }

    /// The point of naming them: an operator can see whether the one they are
    /// waiting for is in the list, which a count cannot tell them.
    #[test]
    fn every_download_is_named_with_its_client_and_how_far_along_it_is() {
        let text = interrupting(&[
            coming(Protocol::Torrent, "Some.Show.S01E04", 68),
            coming(Protocol::Usenet, "Another.Film.2024", 12),
        ])
        .text();

        assert!(text.contains("2 downloads still active:"), "{text}");
        assert!(text.contains("torrent"), "{text}");
        assert!(text.contains("Some.Show.S01E04"), "{text}");
        assert!(text.contains("68%"), "{text}");
        assert!(text.contains("usenet"), "{text}");
        assert!(text.contains("Another.Film.2024"), "{text}");
        assert!(text.contains("12%"), "{text}");
    }

    /// An operator reading "1 downloads" learns the line was assembled rather than
    /// written, and stops trusting the rest of it for the same reason.
    #[test]
    fn one_download_is_said_in_the_singular() {
        let text = interrupting(&[coming(Protocol::Torrent, "Only.One", 3)]).text();

        assert!(text.contains("1 download still active:"), "{text}");
    }

    /// A release name longer than the column is clipped rather than wrapped: the
    /// front of a name is the title, which is what makes it recognisable.
    #[test]
    fn a_long_name_is_clipped_rather_than_wrapped() {
        let long = "A.Really.Very.Long.Release.Name.That.Runs.Past.The.Column.2024.2160p";
        let text = interrupting(&[coming(Protocol::Usenet, long, 50)]).text();

        assert_eq!(
            text.lines().count(),
            3,
            "a blank, the heading, one row: {text}"
        );
        assert!(text.contains("A.Really.Very.Long.Release.Name"), "{text}");
        assert!(
            !text.contains("2160p"),
            "the tail is what gives way: {text}"
        );
    }
}
