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
    let shortened = fitted(uhd, NAMED);
    assert!(shortened.ends_with("WEB-DL"), "{shortened}");
    assert!(shortened.starts_with("A.Very.Long"), "{shortened}");
}
