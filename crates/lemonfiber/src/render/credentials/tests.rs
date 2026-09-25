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
                Propagation::updated("qBittorrent's own web UI"),
                Propagation::pending("the forwarded-port push", "lemonfiber restart torrent"),
                Propagation::failed("Sonarr's download client", "Sonarr did not answer"),
            ],
        )),
    );

    assert!(text.contains("was replaced"), "{text}");
    assert!(text.contains("qBittorrent's own web UI — has it"), "{text}");
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
                detail: "qBittorrent refused the password lemonfiber holds".to_owned(),
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
