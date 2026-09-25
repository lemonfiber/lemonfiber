use super::{Propagation, Reach, Rotation, Settled};

/// A landed rotation over three consumers, one of which could not be reached.
fn partly_landed() -> Rotation {
    Rotation::landed(
        "qBittorrent web UI password",
        "signed in with the replacement",
        vec![
            Propagation::updated("qBittorrent's own web UI"),
            Propagation::pending("the forwarded-port push", "lemonfiber restart torrent"),
            Propagation::failed("Sonarr's download client", "Sonarr did not answer"),
        ],
    )
}

#[test]
fn a_refused_replacement_leaves_the_existing_credential_in_force() {
    let stopped = Rotation::stopped(
        "Indexer API key",
        Settled::Refused {
            detail: "the indexer refused it".to_owned(),
        },
    );

    assert!(stopped.kept_the_existing());
    assert!(stopped.consumers.is_empty());
}

#[test]
fn an_unproven_replacement_leaves_the_existing_credential_in_force() {
    let stopped = Rotation::stopped(
        "Indexer API key",
        Settled::Unproven {
            detail: "nothing answered".to_owned(),
        },
    );

    assert!(stopped.kept_the_existing());
}

#[test]
fn a_name_nothing_answers_to_changes_nothing_and_says_what_would_have() {
    let stopped = Rotation::stopped(
        "the wifi password",
        Settled::Unknown {
            known: vec!["Indexer API key".to_owned()],
        },
    );

    assert!(stopped.kept_the_existing());
    assert!(matches!(
        stopped.settled,
        Settled::Unknown { ref known } if known == &["Indexer API key".to_owned()]
    ));
}

#[test]
fn a_credential_the_operators_provider_issued_is_not_one_lemonfiber_invents() {
    let stopped = Rotation::stopped(
        "Usenet provider password",
        Settled::Elsewhere {
            detail: "change it with your provider first".to_owned(),
        },
    );

    assert!(stopped.kept_the_existing());
}

#[test]
fn only_a_landed_replacement_stops_the_existing_value_being_the_one_in_force() {
    assert!(!partly_landed().kept_the_existing());
}

#[test]
fn a_consumer_that_could_not_be_updated_is_named_rather_than_passed_over() {
    let landed = partly_landed();

    assert_eq!(landed.stranded(), vec!["Sonarr's download client"]);
    assert_eq!(landed.consumers.len(), 3);
}

#[test]
fn a_consumer_waiting_on_one_more_step_counts_as_carrying_the_replacement() {
    assert!(Reach::Updated.carried());
    assert!(Reach::Pending {
        detail: "lemonfiber restart torrent".to_owned()
    }
    .carried());
    assert!(!Reach::Failed {
        detail: "did not answer".to_owned()
    }
    .carried());
}

#[test]
fn a_rotation_that_reached_everything_strands_nobody() {
    let landed = Rotation::landed(
        "Jellyfin administrator password",
        "signed in",
        vec![Propagation::updated("Jellyfin")],
    );

    assert_eq!(landed.consumers.len(), 1);
    assert!(landed.stranded().is_empty());
}
