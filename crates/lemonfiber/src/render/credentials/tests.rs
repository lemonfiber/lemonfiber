use lemonfiber_core::app::Outcome;
use lemonfiber_core::credential::{
    Held, Inventory, Origin, Propagation, Revealed, Rotation, Settled, State, REVEALED, SHOULDER,
};

/// One credential, with the state and advisory a test is about.
fn held(state: State, advisory: Option<&str>) -> Held {
    Held {
        name: "qBittorrent web UI password".to_owned(),
        setting: "QBITTORRENT_PASSWORD".to_owned(),
        consumers: vec!["the tunnel's forwarded-port push".to_owned()],
        location: "/somewhere/.env".to_owned(),
        origin: Origin::Lemonfiber,
        from: lemonfiber_core::origin::Origin::Bundled,
        state,
        fingerprint: None,
        advisory: advisory.map(ToOwned::to_owned),
    }
}

/// The inventory as it reaches a terminal.
fn drawn(inventory: Inventory) -> String {
    crate::render::shaped(&Outcome::Credentials(inventory)).text()
}

/// A value built rather than written, so nothing reads it as a real credential.
fn a_value() -> String {
    format!("{}{}", "the-", "value-itself")
}

#[test]
fn a_plugins_secret_says_which_plugin_brought_it_and_the_stacks_own_say_nothing() {
    let mut theirs = held(State::Absent, None);
    theirs.from = lemonfiber_core::origin::Origin::Plugin {
        named: "comics".to_owned(),
    };
    let text = drawn(Inventory::of(vec![theirs, held(State::Active, None)]));

    assert_eq!(
        text.matches("brought by   the plugin comics").count(),
        1,
        "{text}"
    );
}

#[test]
fn each_credential_says_what_it_is_where_it_is_and_what_uses_it() {
    let text = drawn(Inventory::of(vec![held(State::Active, None)]));

    assert!(
        text.contains("qBittorrent web UI password — active"),
        "{text}"
    );
    assert!(text.contains("QBITTORRENT_PASSWORD"), "{text}");
    assert!(text.contains("/somewhere/.env"), "{text}");
    assert!(text.contains("forwarded-port push"), "{text}");
}

/// The honest half nobody wants to write is printed every time.
#[test]
fn what_the_storage_does_not_protect_against_is_printed_beside_what_it_does() {
    let text = drawn(Inventory::of(vec![held(State::Active, None)]));

    assert!(text.contains("It does not protect against:"), "{text}");
    assert!(text.contains("malware"), "{text}");
    assert!(text.contains("not encrypted"), "{text}");
}

#[test]
fn an_advisory_is_printed_and_says_it_does_not_expire() {
    let text = drawn(Inventory::of(vec![held(
        State::Stale,
        Some("never proven against the service that takes it"),
    )]));

    assert!(text.contains("None of this expires on its own"), "{text}");
    assert!(text.contains("never proven"), "{text}");
}

#[test]
fn nothing_worth_advising_prints_no_advisory_heading() {
    let text = drawn(Inventory::of(vec![held(State::Active, None)]));

    assert!(!text.contains("Worth knowing"), "{text}");
}

#[test]
fn a_landed_rotation_names_every_consumer_and_how_far_it_reached_each() {
    let text = drawn(
        Inventory::of(vec![held(State::Active, None)]).after(Rotation::landed(
            "qBittorrent web UI password",
            "qBittorrent signed in with it",
            vec![
                Propagation::updated("the torrent client's own web UI"),
                Propagation::pending("the forwarded-port push", "lemonfiber restart torrent"),
                Propagation::failed("Sonarr's download client", "Sonarr did not answer"),
            ],
        )),
    );

    assert!(text.contains("was replaced"), "{text}");
    assert!(
        text.contains("the torrent client's own web UI — has it"),
        "{text}"
    );
    assert!(
        text.contains("still to be given it: run `lemonfiber restart torrent`"),
        "{text}"
    );
    assert!(
        text.contains("could not be given it: Sonarr did not answer"),
        "{text}"
    );
    assert!(
        text.contains(
            "Still holding the old value, and it no longer works: Sonarr's \
                       download client."
        ),
        "{text}"
    );
}

#[test]
fn a_refused_rotation_says_so_and_lists_no_consumers() {
    let text = drawn(
        Inventory::of(vec![held(State::Active, None)]).after(Rotation::stopped(
            "qBittorrent web UI password",
            Settled::Refused {
                detail: "the torrent client refused the password lemonfiber holds".to_owned(),
            },
        )),
    );

    assert!(text.contains("was not replaced"), "{text}");
    assert!(
        !text.contains("Everything that authenticates with it"),
        "{text}"
    );
}

#[test]
fn an_unproven_rotation_says_what_stopped_it() {
    let text = drawn(Inventory::of(Vec::new()).after(Rotation::stopped(
        "qBittorrent web UI password",
        Settled::Unproven {
            detail: "no randomness was available".to_owned(),
        },
    )));

    assert!(text.contains("no randomness was available"), "{text}");
}

#[test]
fn a_credential_that_must_be_changed_elsewhere_says_where() {
    let text = drawn(Inventory::of(Vec::new()).after(Rotation::stopped(
        "Usenet provider password",
        Settled::Elsewhere {
            detail: "change it with your provider first".to_owned(),
        },
    )));

    assert!(
        text.contains("Usenet provider password: change it with your provider first"),
        "{text}"
    );
}

#[test]
fn a_replaced_key_that_did_not_answer_is_said_in_the_services_own_terms() {
    let text = drawn(Inventory::of(Vec::new()).after(Rotation::stopped(
        "Sonarr API key",
        Settled::ReplacedUnproven {
            detail: "the service replaced its key and the new one did not answer".to_owned(),
        },
    )));

    assert!(
        text.contains(
            "Sonarr API key: the service replaced its key and the new one did not answer"
        ),
        "{text}"
    );
    assert!(!text.contains("was not replaced"), "{text}");
}

#[test]
fn a_rehearsed_rotation_names_where_the_value_lives_and_what_is_owed_after() {
    let text = drawn(Inventory::of(Vec::new()).after(Rotation::would(
        "qBittorrent web UI password",
        "a real run would generate a new web UI password",
        "the environment file, as QBITTORRENT_PASSWORD",
        vec!["the forwarded-port push — lemonfiber restart torrent".to_owned()],
    )));

    assert!(text.contains("would be replaced"), "{text}");
    assert!(text.contains("QBITTORRENT_PASSWORD"), "{text}");
    assert!(text.contains("lemonfiber restart torrent"), "{text}");
    assert!(
        !text.contains("was replaced"),
        "a rehearsal must not read as a rotation that landed: {text}"
    );
}

/// A rehearsal with nothing owed afterwards prints no heading for it.
///
/// A service's own key is handed out by the rotation itself and by nothing else, so
/// there is genuinely no step left over — and an empty heading reads as a list
/// somebody forgot to fill in, which sends the operator looking for instructions
/// that were never there. The one above prints the heading because it has steps to
/// put under it; the difference between the two is the whole of what the heading is
/// for.
#[test]
fn a_rehearsed_rotation_owing_nothing_afterwards_prints_no_heading_for_it() {
    let text = drawn(Inventory::of(Vec::new()).after(Rotation::would(
        "Sonarr API key",
        "a real run would read the key the service wrote for itself",
        "the environment file, as SONARR_API_KEY",
        Vec::new(),
    )));

    assert!(text.contains("would be replaced"), "{text}");
    assert!(
        !text.contains("Afterwards"),
        "a heading was printed over a list with nothing in it: {text}"
    );
}

#[test]
fn a_name_nothing_answers_to_says_what_would_have() {
    let text = drawn(Inventory::of(Vec::new()).after(Rotation::stopped(
        "the wifi password",
        Settled::Unknown {
            known: vec!["Indexer API key".to_owned()],
        },
    )));

    assert!(
        text.contains("Nothing here is called `the wifi password`"),
        "{text}"
    );
    assert!(text.contains("Indexer API key"), "{text}");
}

#[test]
fn an_unconfirmed_reveal_prints_the_warning_and_no_value() {
    let secret = a_value();
    let text = drawn(Inventory::of(Vec::new()).showing(Revealed {
        name: "Indexer API key".to_owned(),
        value: None,
        warning: SHOULDER.to_owned(),
    }));

    assert!(
        text.contains("scrollback"),
        "the warning was not printed over the reveal"
    );
    assert!(
        !text.contains(&secret),
        "a value was printed under a warning nobody confirmed"
    );
}

#[test]
fn a_confirmed_reveal_prints_the_value_under_the_warning() {
    let secret = a_value();
    let text = drawn(Inventory::of(Vec::new()).showing(Revealed {
        name: "Indexer API key".to_owned(),
        value: Some(secret.clone()),
        warning: REVEALED.to_owned(),
    }));

    assert!(
        text.contains(&format!("Indexer API key: {secret}")),
        "the value a confirmed reveal was asked for was not printed"
    );
    assert!(
        text.contains("Clear your scrollback"),
        "the value was printed with no warning under it"
    );
}

/// The property the whole shape exists for: listing what is held prints no value,
/// whatever is recorded, because there is nowhere in the listing to put one.
#[test]
fn listing_what_is_held_prints_no_value_even_when_one_is_recorded() {
    let secret = a_value();
    let mut line = held(State::Active, None);
    line.fingerprint = Some(lemonfiber_core::credential::fingerprint(&secret));

    let text = drawn(Inventory::of(vec![line]));

    assert!(!text.is_empty());
    assert!(
        !text.contains(&secret),
        "a recorded value was printed in the listing"
    );
}

/// The grant of the torrent client's password to the comics plugin.
fn a_grant() -> lemonfiber_core::credential::Granted {
    lemonfiber_core::credential::Granted {
        plugin: "comics".to_owned(),
        credential: "QBITTORRENT_PASSWORD".to_owned(),
        of: "qbittorrent".to_owned(),
        serves: "download.torrent".to_owned(),
        at: "1".to_owned(),
    }
}

/// Revoking that grant, ended or not, restarting `restarts`.
fn revoked(ended: bool, restarts: &[&str]) -> lemonfiber_core::credential::Revoked {
    lemonfiber_core::credential::Revoked {
        grant: a_grant(),
        restarts: restarts.iter().map(|one| (*one).to_owned()).collect(),
        agreement: "1a2b3c4d".to_owned(),
        ended,
        unrestarted: None,
    }
}

#[test]
fn a_grant_is_listed_beside_the_credential_it_crosses_and_apart_where_none_is_listed() {
    let mut inventory = Inventory::of(vec![held(State::Active, None)]);
    inventory.grants = vec![a_grant()];
    let beside = drawn(inventory.clone());
    inventory.held.clear();
    let apart = drawn(inventory);

    assert!(
        beside.contains("granted to   the plugin comics, for download.torrent"),
        "{beside}"
    );
    assert!(
        !beside.contains("Granted to third-party plugins"),
        "{beside}"
    );
    assert!(apart.contains("Granted to third-party plugins:"), "{apart}");
    assert!(
        apart.contains("QBITTORRENT_PASSWORD, qbittorrent's, to the plugin comics"),
        "{apart}"
    );
}

#[test]
fn a_revoke_not_yet_answered_says_what_it_would_restart_and_the_offer_that_ends_it() {
    let text = drawn(Inventory::of(Vec::new()).revoking(revoked(false, &["comics"])));

    assert!(
        text.contains("would write it out of comics's secrets file"),
        "{text}"
    );
    assert!(text.contains("restart comics"), "{text}");
    assert!(text.contains("--offer 1a2b3c4d"), "{text}");
}

#[test]
fn an_ended_grant_says_what_restarted_and_how_to_take_back_what_was_handed() {
    let text = drawn(Inventory::of(Vec::new()).revoking(revoked(true, &["comics"])));
    let mut unrestarted = revoked(true, &["comics"]);
    unrestarted.unrestarted = Some("the engine did not answer".to_owned());
    let stalled = drawn(Inventory::of(Vec::new()).revoking(unrestarted));

    assert!(
        text.contains("comics's grant of QBITTORRENT_PASSWORD is ended"),
        "{text}"
    );
    assert!(text.contains("Restarted comics."), "{text}");
    assert!(
        text.contains("lemonfiber credentials --rotate QBITTORRENT_PASSWORD"),
        "{text}"
    );
    assert!(
        stalled.contains("did not restart: the engine did not answer"),
        "{stalled}"
    );
}

#[test]
fn a_rehearsed_revoke_of_a_grant_nothing_reads_restarts_nothing_and_writes_nothing() {
    let mut rehearsed = Inventory::of(Vec::new()).revoking(revoked(false, &[]));
    rehearsed.rehearsed = true;
    let text = drawn(rehearsed);
    let ended = drawn(Inventory::of(Vec::new()).revoking(revoked(true, &[])));

    assert!(
        text.contains("restart nothing, since no service of it reads it"),
        "{text}"
    );
    assert!(
        text.contains("Nothing was written, because this was a rehearsal"),
        "{text}"
    );
    assert!(text.contains("without --dry-run"), "{text}");
    assert!(!ended.contains("Restarted"), "{ended}");
    assert!(!ended.contains("did not restart"), "{ended}");
}
