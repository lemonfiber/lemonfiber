use super::{Availability, Suggestion, SUGGESTIONS};
use crate::recyclarr::Kind;

/// Safest first, which is the order the list is held in.
const ORDER: [Availability; 2] = [Availability::FreelyLicensed, Availability::WidelyCarried];

#[test]
fn the_safest_suggestions_come_first() {
    // Freely-licensed titles are mirrored deliberately and by everyone, so they are
    // the likeliest first attempt to succeed. Ordering is the whole guarantee here.
    let ranks: Vec<usize> = SUGGESTIONS
        .iter()
        .map(|suggestion| {
            ORDER
                .iter()
                .position(|a| *a == suggestion.availability)
                .unwrap_or(0)
        })
        .collect();
    let mut sorted = ranks.clone();
    sorted.sort_unstable();
    assert_eq!(ranks, sorted, "freely-licensed titles come first");
}

#[test]
fn a_stack_is_only_suggested_what_it_can_handle() {
    // A stack with no film service should not be offered a film, however safe.
    let television = Suggestion::safest(&[Kind::Sonarr]);
    assert_eq!(television.map(|s| s.kind), Some(Kind::Sonarr));
    let film = Suggestion::safest(&[Kind::Radarr]);
    assert_eq!(film.map(|s| s.kind), Some(Kind::Radarr));
    assert_eq!(
        Suggestion::safest(&[]),
        None,
        "nothing running, nothing to suggest"
    );
    assert!(Suggestion::for_kinds(&[]).is_empty());
    assert_eq!(
        Suggestion::for_kinds(&[Kind::Sonarr, Kind::Radarr]).len(),
        SUGGESTIONS.len(),
        "a stack running both is offered everything"
    );
}

#[test]
fn both_kinds_have_something_to_suggest() {
    // A stack running only one of the two is still walked, so each kind needs at
    // least one thing to try.
    for kind in [Kind::Sonarr, Kind::Radarr] {
        assert!(
            !Suggestion::for_kind(kind).is_empty(),
            "nothing to suggest for {kind:?}"
        );
    }
}

#[test]
fn every_kind_has_something_safe_to_be_asked_for_without_a_maybe() {
    // A caller holding a running service should never have to handle "and if there
    // were nothing to suggest" — a branch it could not reach and could not test.
    for kind in [Kind::Sonarr, Kind::Radarr] {
        let safe = Suggestion::safe_for(kind);
        assert!(
            Suggestion::for_kind(kind).iter().any(|s| s.title == safe),
            "{safe} is not among the suggestions for {kind:?}"
        );
    }
}

#[test]
fn a_suggestion_says_what_it_is_and_why_it_is_safe() {
    let said = SUGGESTIONS
        .first()
        .map(|suggestion| (suggestion.said(), suggestion.title, suggestion.availability));
    assert!(
        said.is_some_and(|(said, title, availability)| said.starts_with(title)
            && said.contains(availability.because())),
        "a suggestion names itself and says why it is safe"
    );
}

#[test]
fn every_reason_a_suggestion_is_safe_reads_as_a_reason() {
    for availability in [Availability::FreelyLicensed, Availability::WidelyCarried] {
        assert!(availability.because().contains("so"), "{availability:?}");
    }
}
