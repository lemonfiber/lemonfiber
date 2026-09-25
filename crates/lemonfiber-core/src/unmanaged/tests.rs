use super::{covering, covers, parse};

/// One declaration, with a reason long enough to be one.
fn declared(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(area, why)| ((*area).to_owned(), (*why).to_owned()))
        .collect()
}

#[test]
fn a_declaration_is_a_name_and_the_reason_somebody_gave() {
    let read = parse("sonarr=I tune this one by hand every season");
    assert_eq!(
        read,
        declared(&[("sonarr", "I tune this one by hand every season")])
    );
}

#[test]
fn several_declarations_are_separated_by_commas_and_trimmed() {
    let read = parse(
        " sonarr = I tune this one by hand every season , \
         config/recyclarr = my own profiles live in here ",
    );
    assert_eq!(read.len(), 2, "{read:?}");
    assert!(read.iter().any(|(area, _)| area == "config/recyclarr"));
}

/// Both halves, and neither blank. How good the reason is is not judged: an entry
/// dropped here is somebody who wrote something down, watched their file be
/// rewritten anyway, and has nothing telling them their sentence was too short.
#[test]
fn an_entry_missing_either_half_declares_nothing() {
    assert!(parse("sonarr").is_empty(), "no reason at all");
    assert!(parse("sonarr=").is_empty(), "a blank reason");
    assert!(parse("=I have my reasons").is_empty(), "no name");
    assert!(parse("").is_empty(), "nothing written down");
}

/// And a thin reason still declares, which is the asymmetry with the register of
/// deliberately exposed services: that one silences a warning and this one stops a
/// write, so they fail in opposite directions on purpose.
#[test]
fn a_reason_nobody_would_praise_is_still_a_declaration() {
    assert_eq!(parse("sonarr=mine"), declared(&[("sonarr", "mine")]));
}

#[test]
fn a_declared_name_covers_itself_and_says_why() {
    let theirs = declared(&[("sonarr", "I tune this one by hand every season")]);
    assert_eq!(
        covering(&theirs, "sonarr"),
        Some("I tune this one by hand every season")
    );
    assert!(covers(&theirs, "sonarr"));
}

#[test]
fn a_declared_name_covers_what_sits_beneath_it() {
    let theirs = declared(&[("config/recyclarr", "my own profiles live in here")]);
    assert!(covers(&theirs, "config/recyclarr/recyclarr.yml"));
    assert!(covers(&theirs, "config/recyclarr/includes/mine.yml"));
}

/// A name that merely begins with another name is a different name.
#[test]
fn a_declared_name_does_not_cover_its_neighbours() {
    let theirs = declared(&[("config", "everything under here is mine to keep")]);
    assert!(!covers(&theirs, "configuration.yml"));
    assert!(!covers(&theirs, "config.yml"));
    assert!(covers(&theirs, "config/recyclarr/recyclarr.yml"));
}

#[test]
fn nothing_declared_covers_nothing() {
    assert!(!covers(&[], "sonarr"));
    assert_eq!(covering(&[], "sonarr"), None);
}

/// Nothing is inferred from the shape of a name, so a service and a path are the
/// same kind of thing to this — which is what lets a fork call a service `config`.
#[test]
fn a_name_is_a_name_whether_it_looks_like_a_path_or_a_service() {
    let theirs = declared(&[
        ("DATA_ROOT", "I move the library about by hand"),
        ("compose.yml", "I keep my own edits in this file"),
    ]);
    assert!(covers(&theirs, "DATA_ROOT"));
    assert!(covers(&theirs, "compose.yml"));
    assert!(!covers(&theirs, "DATA_ROOT_BACKUP"));
}
