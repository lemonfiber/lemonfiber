use super::LookupResult;
use crate::recyclarr::Kind;

/// A catalogue result the way each service sends one.
fn result(id: i64, television: Option<i64>, film: Option<i64>) -> LookupResult {
    LookupResult {
        id,
        title: "Sintel".to_owned(),
        year: Some(2010),
        tvdb_id: television,
        tmdb_id: film,
    }
}

#[test]
fn each_service_is_read_by_its_own_identifier() {
    // Sonarr files by TVDB and Radarr by TMDB; reading the wrong one would produce
    // an entry that looks addable and is not.
    assert_eq!(result(0, Some(77), None).entry(Kind::Sonarr).reference, 77);
    assert_eq!(result(0, None, Some(99)).entry(Kind::Radarr).reference, 99);
    assert_eq!(
        result(0, Some(77), None).entry(Kind::Radarr).reference,
        0,
        "a television id is not a film id"
    );
}

#[test]
fn a_zero_id_is_the_catalogue_saying_it_does_not_hold_this() {
    // The distinction the whole already-present detection rests on.
    assert!(!result(0, Some(1), None)
        .entry(Kind::Sonarr)
        .is_already_here());
    assert_eq!(
        result(42, Some(1), None).entry(Kind::Sonarr).held_as,
        Some(42)
    );
}

#[test]
fn a_result_keeps_the_year_that_tells_two_of_a_name_apart() {
    let entry = result(0, Some(1), None).entry(Kind::Sonarr);
    assert_eq!(entry.named(), "Sintel (2010)");
}
