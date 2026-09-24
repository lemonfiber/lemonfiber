use super::{offered, offers, BESIDE_THE_POINTS};
use crate::extension;

/// What a point asks for is what this build offers by publishing the point.
#[test]
fn every_published_point_is_an_offer() {
    for point in extension::POINTS {
        assert!(
            offers(point.requires),
            "{} asks for {}, which nothing offers",
            point.name,
            point.requires
        );
    }
    assert!(
        !extension::POINTS.is_empty(),
        "a build publishing no point at all would make that sweep vacuous"
    );
}

/// A capability nothing here provides is not offered, whatever a manifest asks.
#[test]
fn a_capability_no_mechanism_here_provides_is_not_offered() {
    assert!(!offers("service.add"));
    assert!(!offers("recipe.run"));
}

/// The set is derived, so removing what it is derived from empties it.
///
/// The property worth holding is that this cannot silently answer yes to
/// everything: with nothing to derive from and nothing written beside it, the
/// answer to every name is no.
#[test]
fn with_no_point_and_nothing_beside_them_nothing_is_offered() {
    assert_eq!(
        BESIDE_THE_POINTS.len() + extension::POINTS.len(),
        offered().len() + repeated(),
        "the offered set is exactly what it is derived from"
    );
}

/// How many points share a capability with another point or with a listed offer.
fn repeated() -> usize {
    let derived: Vec<&str> = extension::POINTS
        .iter()
        .map(|point| point.requires)
        .chain(BESIDE_THE_POINTS.iter().copied())
        .collect();
    derived.len() - offered().len()
}
