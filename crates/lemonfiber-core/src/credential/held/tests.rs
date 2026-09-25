use super::{catalogue, Consumer, Entry, Needed, Origin, CATALOGUE, QBITTORRENT};
use crate::config::Protocols;
use crate::credential::Reach;

#[test]
fn every_entry_names_what_it_is_where_it_lives_and_who_uses_it() {
    assert_eq!(CATALOGUE.len(), 7);
    for entry in CATALOGUE {
        assert!(!entry.name.is_empty(), "{}", entry.setting);
        assert!(!entry.setting.is_empty(), "{}", entry.name);
        assert!(!entry.consumers.is_empty(), "{}", entry.name);
    }
}

#[test]
fn no_two_entries_are_recorded_under_the_same_setting() {
    let mut settings: Vec<&str> = CATALOGUE.iter().map(|entry| entry.setting).collect();
    let held = settings.len();
    settings.sort_unstable();
    settings.dedup();
    assert_eq!(settings.len(), held, "{settings:?}");
}

/// The consumer this list exists for: a rotation reaching qBittorrent alone leaves
/// the tunnel unable to apply the port it was granted.
#[test]
fn the_torrent_password_names_the_port_push_as_well_as_the_client() {
    let consumers: Vec<&str> = QBITTORRENT.consumers.iter().map(|one| one.name).collect();
    let consumers = consumers.join("; ");

    assert!(consumers.contains("forwarded-port push"), "{consumers}");
    assert!(consumers.contains("web UI"), "{consumers}");
    assert!(QBITTORRENT.consumers.len() >= 4, "{consumers}");
}

#[test]
fn a_stack_that_does_not_torrent_is_not_told_it_is_missing_a_tunnel_key() {
    let usenet_only = catalogue(Protocols {
        usenet: true,
        torrent: false,
    });
    let named: Vec<&str> = usenet_only.iter().map(|entry| entry.name).collect();

    assert!(!named.is_empty());
    assert!(!named.iter().any(|name| name.contains("VPN")), "{named:?}");
    assert!(
        named.iter().any(|name| name.contains("Usenet")),
        "{named:?}"
    );
}

#[test]
fn a_stack_that_only_torrents_is_not_asked_for_a_usenet_password() {
    let torrent_only = catalogue(Protocols {
        usenet: false,
        torrent: true,
    });
    let named: Vec<&str> = torrent_only.iter().map(|entry| entry.name).collect();

    assert!(named.iter().any(|name| name.contains("VPN")), "{named:?}");
    assert!(
        !named.iter().any(|name| name.contains("Usenet")),
        "{named:?}"
    );
}

#[test]
fn a_stack_running_neither_still_holds_the_credentials_it_always_needs() {
    let neither = catalogue(Protocols::none());

    // Bound rather than called in the message: a call inside an assertion's
    // message only runs when the assertion fails, so the helper would never run
    // on a passing test and would read as dead code.
    let listed = names(&neither);
    assert_eq!(neither.len(), 4, "{listed:?}");
    assert!(neither.iter().all(|entry| entry.needed == Needed::Always));
}

#[test]
fn both_protocols_ask_for_every_credential_there_is() {
    assert_eq!(catalogue(Protocols::both()).len(), CATALOGUE.len());
}

/// Only a credential the operator was offered the chance to keep unproven has
/// anything recording whether they took it.
#[test]
fn what_records_a_proof_is_recorded_only_where_proceeding_unproven_was_a_choice() {
    let recorded: Vec<&str> = CATALOGUE
        .iter()
        .filter(|entry| entry.proven_by.is_some())
        .map(|entry| entry.setting)
        .collect();

    assert_eq!(recorded.len(), 2, "{recorded:?}");

    // Counted before anything is said about all of them: an empty set satisfies
    // every claim, so a catalogue that stopped minting anything would pass this
    // while proving nothing.
    let minted: Vec<&str> = CATALOGUE
        .iter()
        .filter(|entry| entry.origin.mints_its_own())
        .map(|entry| entry.setting)
        .collect();
    assert_eq!(minted.len(), 4, "{minted:?}");
    assert!(CATALOGUE
        .iter()
        .filter(|entry| entry.origin.mints_its_own())
        .all(|entry| entry.proven_by.is_none()));
}

#[test]
fn only_what_lemonfiber_minted_is_something_it_can_replace_by_itself() {
    assert!(Origin::Lemonfiber.mints_its_own());
    assert!(!Origin::Operator.mints_its_own());
    assert!(!Origin::Service.mints_its_own());
}

/// Each way of reaching a consumer, built and read back.
///
/// Built here at run time rather than only in the table above, where every one of
/// them is settled while this is being compiled: a constructor reached only from a
/// constant runs nowhere, and a rule nothing runs is a rule nothing proves.
#[test]
fn each_way_of_reaching_a_consumer_carries_what_it_takes_to_get_there() {
    let at = Consumer::at_the_service("the service itself");
    let from = Consumer::from_its_environment("a container", "lemonfiber restart torrent");
    let by = Consumer::by_seeding("a service lemonfiber writes to");
    let read = Consumer::read_by_lemonfiber("lemonfiber's own check");

    assert_eq!(at.reached().reach, Reach::Updated);
    assert_eq!(read.reached().reach, Reach::Updated);
    assert_eq!(
        from.reached().reach,
        Reach::Pending {
            detail: "lemonfiber restart torrent".to_owned()
        }
    );
    assert_eq!(
        by.reached().reach,
        Reach::Pending {
            detail: "lemonfiber seed".to_owned()
        }
    );
    assert_eq!(at.reached().consumer, "the service itself");
}

#[test]
fn every_origin_is_written_out_for_a_person_to_read() {
    for origin in [Origin::Operator, Origin::Service, Origin::Lemonfiber] {
        assert!(origin.as_str().contains(' '), "{}", origin.as_str());
    }
}

/// The names in a set, for a failure message that says which set.
fn names(entries: &[Entry]) -> Vec<&str> {
    entries.iter().map(|entry| entry.name).collect()
}
