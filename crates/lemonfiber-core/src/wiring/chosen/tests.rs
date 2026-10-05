use super::Chosen;

/// Words an operator might type that would break a plain `capability=reason` list.
const AWKWARD: &str = "Plex has my old library, & it=works; 100% sure, really";

#[test]
fn a_reason_reads_back_as_it_was_typed() {
    let chosen = Chosen::read(Some("media.serve=plex"));
    let setting = chosen.reasons_with("media.serve", Some(AWKWARD));
    let read = Chosen::read(Some("media.serve=plex")).because(setting.as_deref());
    assert_eq!(read.why("media.serve"), Some(AWKWARD));
}

#[test]
fn a_choice_made_with_no_reason_takes_the_last_one_away() {
    let before = Chosen::read(Some("media.serve=plex,indexer.search=nzbhydra2"))
        .because(Some("media.serve=old&indexer.search=kept"));
    let setting = before.reasons_with("media.serve", None);
    let after = Chosen::read(Some("media.serve=jellyfin,indexer.search=nzbhydra2"))
        .because(setting.as_deref());
    assert_eq!(after.why("media.serve"), None);
    assert_eq!(after.why("indexer.search"), Some("kept"));
}

#[test]
fn an_empty_reason_is_no_reason() {
    let chosen = Chosen::read(Some("media.serve=plex"));
    assert_eq!(chosen.reasons_with("media.serve", Some("")), None);
}

#[test]
fn a_reason_for_a_capability_nobody_chose_for_is_not_read() {
    // It explains a choice that is not there, and reading it back would put the
    // operator's words beside the stack's own settlement.
    let chosen = Chosen::read(Some("media.serve=plex")).because(Some("indexer.search=stray"));
    assert_eq!(chosen.why("indexer.search"), None);
    assert_eq!(chosen.reasons(), None);
}

#[test]
fn the_reasons_a_setting_holds_are_written_back_as_they_were_read() {
    let setting = "indexer.search=b&media.serve=a";
    let chosen =
        Chosen::read(Some("media.serve=plex,indexer.search=nzbhydra2")).because(Some(setting));
    assert_eq!(chosen.reasons().as_deref(), Some(setting));
}
