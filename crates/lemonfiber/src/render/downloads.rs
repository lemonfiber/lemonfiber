//! What is still coming down, said in a way an operator can act on.

use lemonfiber_core::app::Interrupted;
use lemonfiber_core::dashboard::Protocol;
use lemonfiber_core::plural::s;

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

/// A name shortened to fit, keeping both of its ends.
///
/// Elided in the middle rather than cut at the end, because the end of a release
/// name is where the things that tell two of them apart live — the resolution, the
/// encoding, the group. Cut at the tail, `…1080p` and `…2160p` read identically,
/// and a list of what is still downloading that cannot tell two downloads apart
/// fails at the one question it exists to answer.
///
/// The marker is three full stops rather than an ellipsis, so a terminal that
/// cannot render the character is not handed one.
fn fitted(name: &str, width: usize) -> String {
    let counted = name.chars().count();
    if counted <= width {
        return name.to_owned();
    }
    let keep = width.saturating_sub(3);
    let tail = keep / 2;
    let head = keep - tail;
    let front: String = name.chars().take(head).collect();
    let back: String = name.chars().skip(counted - tail).collect();
    format!("{front}...{back}")
}

#[cfg(test)]
mod tests {
    use super::{fitted, interrupting, protocol, NAMED};
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

    /// The defect this guards against: cut at the tail, two releases that differ
    /// only in resolution read identically, and a list of what is downloading that
    /// cannot tell them apart fails at the one question it exists to answer.
    #[test]
    fn two_releases_differing_only_at_the_end_stay_distinguishable() {
        let hd = "A.Very.Long.Release.Name.From.Some.Group.2024.1080p.WEB-DL";
        let uhd = "A.Very.Long.Release.Name.From.Some.Group.2024.2160p.WEB-DL";

        assert_ne!(
            fitted(hd, NAMED),
            fitted(uhd, NAMED),
            "both were shortened to the same thing"
        );
        assert!(
            fitted(uhd, NAMED).ends_with("WEB-DL"),
            "{}",
            fitted(uhd, NAMED)
        );
        assert!(
            fitted(uhd, NAMED).starts_with("A.Very.Long"),
            "{}",
            fitted(uhd, NAMED)
        );
    }

    /// A name that fits is left exactly as it is — shortening one that needs no
    /// shortening would be inventing a change to it.
    #[test]
    fn a_name_that_fits_is_left_alone() {
        assert_eq!(fitted("Short.Name", NAMED), "Short.Name");
        assert_eq!(fitted(&"x".repeat(NAMED), NAMED), "x".repeat(NAMED));
    }

    /// Never wider than asked for, however it was shortened.
    #[test]
    fn a_shortened_name_still_fits_the_column() {
        let long = "z".repeat(NAMED * 3);
        assert_eq!(fitted(&long, NAMED).chars().count(), NAMED);
    }

    /// The marker is full stops rather than an ellipsis, so a terminal that cannot
    /// render the character is never handed one.
    #[test]
    fn shortening_uses_no_character_a_terminal_might_not_have() {
        let text = fitted(&"y".repeat(NAMED * 2), NAMED);

        assert!(text.contains("..."), "{text}");
        assert!(text.is_ascii(), "{text}");
    }
}
