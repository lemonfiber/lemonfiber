use super::Seen;

/// What a source answered — wrapped, since a source that could not be reached
/// answers `None` and that distinction is what half these tests are about.
fn said(address: &str) -> Option<String> {
    (!address.is_empty()).then(|| address.to_owned())
}

#[test]
fn sources_that_agree_settle_the_address() {
    let seen = Seen::of(&[said("203.0.113.7"), said("203.0.113.7")]);
    assert_eq!(seen, Seen::Agreed("203.0.113.7".to_owned()));
    assert_eq!(seen.settled(), Some("203.0.113.7"));
    assert_eq!(seen.said(), None, "nothing to report");
}

#[test]
fn sources_that_disagree_settle_nothing_and_say_so() {
    // Picking a winner would be inventing the answer: there is no basis to
    // prefer one stranger's account over another's, and every verdict
    // downstream would inherit the invention without knowing.
    let seen = Seen::of(&[said("203.0.113.7"), said("198.51.100.9")]);
    assert_eq!(seen.settled(), None);
    let reported = seen.said().unwrap_or_default();
    assert!(reported.contains("203.0.113.7"), "{reported}");
    assert!(reported.contains("198.51.100.9"), "{reported}");
}

#[test]
fn a_source_that_could_not_be_reached_is_silence_rather_than_a_contradiction() {
    // Otherwise a conflict would be reported every time an echo went down,
    // which is often and means nothing.
    let seen = Seen::of(&[said("203.0.113.7"), None]);
    assert_eq!(seen, Seen::Agreed("203.0.113.7".to_owned()));
    assert_eq!(seen.settled(), Some("203.0.113.7"));
}

#[test]
fn nothing_answering_at_all_is_its_own_state() {
    // Distinct from agreement on purpose: no address is not the same claim as
    // an address everybody confirmed, and the verdict differs.
    let seen = Seen::of(&[None, None]);
    assert_eq!(seen, Seen::Silent);
    assert_eq!(seen.settled(), None);
    assert_eq!(seen.said(), None, "silence is not a disagreement to report");
    assert_eq!(Seen::of(&[]), Seen::Silent);
}

#[test]
fn one_source_answering_alone_is_taken_at_its_word() {
    // Better than nothing, and the operator configured only one. The check is
    // then as trustworthy as that source, which is why more than one is asked.
    assert_eq!(
        Seen::of(&[said("203.0.113.7")]).settled(),
        Some("203.0.113.7")
    );
}

#[test]
fn three_sources_with_one_dissenter_is_still_a_disagreement() {
    // Not a vote. Two against one is not evidence about which is right — a
    // majority of strangers is still strangers, and the one dissenting may be
    // the only one not behind a cache.
    let seen = Seen::of(&[
        said("203.0.113.7"),
        said("203.0.113.7"),
        said("198.51.100.9"),
    ]);
    assert_eq!(seen.settled(), None);
    assert!(matches!(seen, Seen::Disagreed(ref heard) if heard.len() == 2));
}
