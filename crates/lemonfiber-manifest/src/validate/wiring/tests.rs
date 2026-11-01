use crate::validate::tests::today;
use crate::{validate, Manifest};

/// The stack this build ships.
const SHIPPED: &str = lemonfiber_bundled::STACK;

/// Everything wrong with a manifest, as one line each.
fn messages(text: &str) -> Vec<String> {
    Manifest::from_toml(text)
        .ok()
        .map(|manifest| {
            validate(&manifest, today())
                .iter()
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Whether the stack this binary carries, edited once, says what it should.
fn fault(from: &str, to: &str, expected: &str) {
    assert!(
        SHIPPED.contains(from),
        "the shipped stack must contain {from:?}"
    );
    let said = messages(&SHIPPED.replacen(from, to, 1));
    assert!(
        said.iter().any(|message| message.contains(expected)),
        "expected {expected:?} among {said:?}"
    );
}

#[test]
fn a_by_name_wiring_that_does_not_say_why_is_caught() {
    fault(
        "by = \"recyclarr\"\nto = \"radarr\"\nwhy = ",
        "by = \"recyclarr\"\nto = \"radarr\"\n# why = ",
        "names radarr and does not say why",
    );
}

/// A reason somebody filled in with spaces is not a reason.
#[test]
fn a_by_name_wiring_whose_reason_is_blank_is_caught() {
    fault(
        "why = \"The same, in Lidarr's terms.\"",
        "why = \"   \"",
        "names lidarr and does not say why",
    );
}

/// Both ends is an ask whose answer was decided in advance, which is a name
/// written the long way.
#[test]
fn a_wiring_that_asks_and_names_is_caught() {
    fault(
        "asks = \"media.serve\"\nby = \"door\"",
        "asks = \"media.serve\"\nby = \"door\"\nto = \"jellyfin\"",
        "a wiring does one or the other",
    );
}

#[test]
fn a_wiring_with_neither_end_is_caught() {
    fault(
        "asks = \"media.serve\"\nby = \"door\"",
        "by = \"door\"",
        "neither asks for a capability nor names a service",
    );
}

#[test]
fn a_wiring_from_something_that_is_not_a_service_is_caught() {
    fault(
        "asks = \"media.serve\"\nby = \"door\"",
        "asks = \"media.serve\"\nby = \"subtitler\"",
        "by names subtitler, which is not a service",
    );
}

#[test]
fn a_wiring_to_something_that_is_not_a_service_is_caught() {
    fault(
        "by = \"unpackerr\"\nto = \"lidarr\"",
        "by = \"unpackerr\"\nto = \"lidaarr\"",
        "to names lidaarr, which is not a service",
    );
}

/// A plugin's own name has a colon and is that plugin's to declare. A link asking
/// for one is asking for something no vocabulary could carry.
#[test]
fn a_wiring_asking_for_a_name_of_the_wrong_shape_is_caught() {
    fault(
        "asks = \"media.serve\"\nby = \"door\"",
        "asks = \"plex:identity\"\nby = \"door\"",
        "asks for plex:identity, which is not a core capability name",
    );
}

/// Choosing a claimant that does not claim it is choosing nothing, and the link
/// would resolve to a service that cannot do what was asked.
#[test]
fn a_chosen_filler_that_does_not_provide_it_is_caught() {
    fault(
        "asks = \"indexer.search\"\nby = \"bindery\"\nfilled_by = \"prowlarr\"",
        "asks = \"indexer.search\"\nby = \"bindery\"\nfilled_by = \"sabnzbd\"",
        "chooses sabnzbd to fill indexer.search, which it does not provide",
    );
}

/// A choice with no reason beside it is indistinguishable from a rule somebody
/// encoded, which is the thing being chosen instead of.
#[test]
fn a_chosen_filler_with_no_reason_is_caught() {
    fault(
        "filled_by = \"prowlarr\"\nwhy = ",
        "filled_by = \"prowlarr\"\n# why = ",
        "chooses prowlarr and does not say why",
    );
}

#[test]
fn a_wiring_that_reaches_all_of_them_and_chooses_one_is_caught() {
    fault(
        "asks = \"media.serve\"\nby = \"door\"\nfilled_by",
        "asks = \"media.serve\"\nby = \"door\"\neach = true\nfilled_by",
        "it does both only by meaning neither",
    );
}

#[test]
fn a_by_name_wiring_carrying_what_only_an_ask_may_say_is_caught() {
    fault(
        "by = \"unpackerr\"\nto = \"sonarr\"",
        "by = \"unpackerr\"\nto = \"sonarr\"\neach = true",
        "carries each, which says something only an ask can say",
    );
}

#[test]
fn a_wiring_from_a_service_to_itself_is_caught() {
    fault(
        "by = \"unpackerr\"\nto = \"radarr\"",
        "by = \"unpackerr\"\nto = \"unpackerr\"",
        "names the service it runs from; nothing wires to itself",
    );
}

#[test]
fn the_same_ask_written_twice_is_caught() {
    fault(
        "asks = \"indexer.search\"\nby = \"bindery\"",
        "asks = \"indexer.search\"\nby = \"bindery\"\n\n[[wiring]]\nasks = \"indexer.search\"\n\
         by = \"bindery\"",
        "names indexer.search more than once",
    );
}

/// The stack this binary ships declares links and every one of them holds.
#[test]
fn every_link_the_shipped_stack_declares_is_sound() {
    assert_eq!(messages(SHIPPED), Vec::<String>::new());
}

/// A stack written before this table existed is still a stack, and one that
/// declares no link declares none rather than being wrong about all of them.
#[test]
fn a_stack_that_declares_no_wiring_at_all_is_faulted_for_none_of_it() {
    let bare = "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"0.1.0\"\n";
    assert_eq!(messages(bare), Vec::<String>::new());
}
