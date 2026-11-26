//! What an interrupted setup keeps, and how what it wrote is put back or run again.

use super::*;
use crate::instant::Instant;
use lemonfiber_fixtures::ports::STAMPED;

/// A saved progress sitting at a given lifecycle phase.
fn saved_at(phase: Phase) -> Progress {
    Progress {
        phase,
        ..Progress::default()
    }
}

/// The reversal of writing a fresh `.env` key: remove it again, where `wrote` is
/// what the apply put there.
fn removed(key: &str, wrote: &str) -> Undo {
    Undo {
        target: ".env".to_owned(),
        action: Action::Restore {
            key: key.to_owned(),
            value: None,
            wrote: wrote.to_owned(),
        },
    }
}

/// The two writes an apply had managed to make before it was interrupted.
fn partial_apply() -> Journal {
    Journal::replay(vec![
        a_fresh_write("DATA_ROOT", "/srv/media"),
        a_fresh_write("USENET", "on"),
    ])
}

#[test]
fn a_fresh_setup_is_in_the_gathering_phase() {
    assert_eq!(Phase::default(), Phase::InProgress);
    assert_eq!(Progress::default().phase, Phase::InProgress);
}

#[test]
fn a_progress_file_predating_the_phase_field_reads_as_gathering() {
    // A file written before the lifecycle was tracked carries no phase; it was
    // only ever left mid-gathering, so it must read back as that rather than
    // fail to load.
    let old = r#"{"at":"protocols","answers":{}}"#;
    let restored = serde_json::from_str::<Progress>(old).ok();
    assert_eq!(
        restored.map(|progress| (progress.phase, progress.at)),
        Some((Phase::InProgress, Step::Protocols)),
    );
}

#[test]
fn the_phase_survives_a_round_trip() {
    for phase in [
        Phase::InProgress,
        Phase::Reviewing,
        Phase::Applying,
        Phase::Applied,
    ] {
        let line = serde_json::to_string(&saved_at(phase)).unwrap_or_default();
        let read = serde_json::from_str::<Progress>(&line).ok();
        assert_eq!(read.map(|progress| progress.phase), Some(phase), "{line}");
    }
}

#[test]
fn no_saved_setup_is_absent() {
    assert_eq!(Status::of(None), Status::Absent);
}

#[test]
fn each_stored_phase_maps_to_its_status() {
    assert_eq!(
        Status::of(Some(&saved_at(Phase::InProgress))),
        Status::InProgress,
    );
    assert_eq!(
        Status::of(Some(&saved_at(Phase::Reviewing))),
        Status::Reviewing,
    );
    assert_eq!(Status::of(Some(&saved_at(Phase::Applied))), Status::Applied);
}

#[test]
fn a_persisted_applying_marker_is_a_failed_apply() {
    // The only writer of the applying marker is a live apply, so reading it
    // back off disk means that apply stopped before it finished.
    assert_eq!(
        Status::of(Some(&saved_at(Phase::Applying))),
        Status::FailedApply,
    );
}

#[test]
fn recovery_reports_exactly_what_the_interrupted_apply_wrote() {
    // The report is the journal's writes unaltered and in order — the data
    // root first, then the protocol toggle.
    let journal = partial_apply();
    assert_eq!(
        Recovery::of(&journal).written(),
        [
            a_fresh_write("DATA_ROOT", "/srv/media"),
            a_fresh_write("USENET", "on")
        ],
    );
}

#[test]
fn a_moved_setting_is_said_by_both_its_names() {
    assert_eq!(
        described(&Change {
            at: Instant::EPOCH,
            operation: "settings".to_owned(),
            target: "MEDIA_SERVER_MODE".to_owned(),
            kind: Kind::Renamed {
                from: "JELLYFIN_MODE".to_owned(),
                to: "MEDIA_SERVER_MODE".to_owned(),
            },
        }),
        "the move of the setting JELLYFIN_MODE to MEDIA_SERVER_MODE"
    );
}

#[test]
fn a_written_change_is_said_plainly_enough_to_recognise() {
    // What whoever is choosing is shown of an interrupted run: the point is
    // that they recognise it, not that it round-trips.
    let change = |kind| Change {
        at: Instant::EPOCH,
        operation: "setup".to_owned(),
        target: "the environment file".to_owned(),
        kind,
    };
    assert_eq!(
        described(&change(Kind::Set {
            key: "DATA_ROOT".to_owned(),
            previous: None,
            current: "/srv".to_owned(),
        })),
        "the setting DATA_ROOT"
    );
    assert_eq!(
        described(&change(Kind::Made {
            path: "/srv/media".to_owned()
        })),
        "the directory /srv/media"
    );
    assert_eq!(
        described(&change(Kind::Created {
            resource: "root folder".to_owned(),
            id: "1".to_owned(),
        })),
        "a root folder"
    );
    assert_eq!(
        described(&change(Kind::KeyMinted {
            name: "ha".to_owned(),
            scope: "read".to_owned(),
        })),
        "the key ha"
    );
    assert_eq!(
        described(&change(Kind::KeyRevoked {
            name: "ha".to_owned(),
            scope: "read".to_owned(),
        })),
        "the revoke of the key ha"
    );

    assert_eq!(
        described(&change(Kind::Setting {
            name: "komga:mode".to_owned(),
            previous: None,
            current: Some("fast".to_owned()),
            filed: None,
            default: None,
        })),
        "the setting komga:mode"
    );
    assert_eq!(
        described(&change(Kind::Secret {
            name: "komga:token".to_owned(),
            from: "recipe".to_owned(),
            previous: None,
            current: Some("~1".to_owned()),
            filed: None,
        })),
        "the secret komga:token"
    );
    assert_eq!(
        described(&change(Kind::Region {
            path: "/srv/stack/config/caddy/Caddyfile".to_owned(),
            key: "config/caddy/Caddyfile".to_owned(),
            owner: "plugin komga".to_owned(),
            written: 0,
        })),
        "plugin komga's region in /srv/stack/config/caddy/Caddyfile"
    );
    // An update's own record, found for the same reason as a repair's: the journal
    // is one file, and what a recovery offers to leave alone has to have a name.
    assert_eq!(
        described(&change(Kind::Pinned {
            previous: "4.0.14".to_owned(),
            current: "4.0.15".to_owned(),
            backup: None,
        })),
        "the move to 4.0.15"
    );
    // A repair's own record. Not something a first run writes, but the journal
    // is shared, so an interrupted setup can find one — and whoever is deciding
    // whether to roll back is owed a name for it rather than silence.
    assert_eq!(
        described(&change(Kind::Configured {
            resource: "downloadclient".to_owned(),
            id: "7".to_owned(),
            field: "tvCategory".to_owned(),
            previous: None,
            current: "tv-sonarr".to_owned(),
        })),
        "a downloadclient's tvCategory"
    );
    // A choice of filler's own record, found for the same reason: it writes over a
    // plugin's document, and the journal is one file.
    assert_eq!(
        described(&change(Kind::Rewritten {
            path: "/srv/stack/compose/plugins/komga.yml".to_owned(),
            previous: String::new(),
            written: 0,
        })),
        "the file /srv/stack/compose/plugins/komga.yml"
    );
}

#[test]
fn resuming_keeps_every_write_already_made() {
    let journal = partial_apply();
    assert_eq!(
        Recovery::of(&journal).resolve(Choice::Resume),
        Resolution::Resume,
    );
}

#[test]
fn rolling_back_reverses_the_writes_most_recent_first() {
    // The later write is undone before the earlier one, and each set with no
    // prior value is removed — the reversal of a two-step partial apply.
    let journal = partial_apply();
    assert_eq!(
        Recovery::of(&journal).resolve(Choice::RollBack),
        Resolution::RollBack(vec![
            removed("USENET", "on"),
            removed("DATA_ROOT", "/srv/media")
        ]),
    );
}

#[test]
fn starting_over_also_reverses_the_writes_before_discarding_them() {
    // Start over is not a bare discard: the partial apply is unwound first,
    // most recent write to earliest, so nothing is stranded on disk — the
    // reversal is the same as a roll back's; only what follows it differs.
    let journal = partial_apply();
    assert_eq!(
        Recovery::of(&journal).resolve(Choice::StartOver),
        Resolution::StartOver(vec![
            removed("USENET", "on"),
            removed("DATA_ROOT", "/srv/media")
        ]),
    );
}

#[test]
fn a_setup_moves_review_to_apply_to_applied_along_its_edges() {
    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    assert_eq!(wizard.phase(), Phase::InProgress);
    assert!(wizard.transition(Phase::Reviewing));
    assert!(wizard.transition(Phase::Applying));
    assert!(wizard.transition(Phase::Applied));
    assert_eq!(wizard.phase(), Phase::Applied);
    // A finished apply cannot be re-run, nor walked back to an earlier phase,
    // so a reached phase is never quietly rewritten.
    assert!(!wizard.transition(Phase::Applying));
    assert!(!wizard.transition(Phase::Reviewing));
    assert_eq!(wizard.phase(), Phase::Applied);
}

#[test]
fn apply_cannot_be_reached_without_passing_review() {
    // The gate review stands for — every question answered — cannot be skipped
    // by jumping a fully-answered wizard straight to applying.
    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    assert!(!wizard.transition(Phase::Applying));
    assert!(!wizard.transition(Phase::Applied));
    assert_eq!(wizard.phase(), Phase::InProgress);
}

#[test]
fn review_is_refused_until_every_question_is_answered() {
    let mut wizard = on_native_linux();
    assert!(!wizard.transition(Phase::Reviewing));
    assert_eq!(wizard.phase(), Phase::InProgress);

    answer_all(&mut wizard);
    assert!(wizard.transition(Phase::Reviewing));
    assert_eq!(wizard.phase(), Phase::Reviewing);
}

#[test]
fn a_rolled_back_apply_returns_to_review_to_be_run_again() {
    // The one backward edge: an apply that was unwound goes back to review,
    // its answers intact, ready to apply once more.
    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    assert!(wizard.transition(Phase::Reviewing));
    assert!(wizard.transition(Phase::Applying));
    assert!(wizard.transition(Phase::Reviewing));
    assert_eq!(wizard.phase(), Phase::Reviewing);
}

#[test]
fn applying_the_plan_records_each_setting_against_what_was_there() {
    let mut wizard = on_native_linux();
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    // The file already carries a usenet setting and nothing about torrents, so
    // one change has a previous value to restore and the other has none.
    let current = EnvFile::parse("LEMONFIBER_USENET=off\n");
    let set = |key: &str, previous: Option<&str>, value: &str| Change {
        at: STAMPED,
        operation: "apply".to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: previous.map(str::to_owned),
            current: value.to_owned(),
        },
    };
    assert_eq!(
        wizard.plan().changes(&current, STAMPED),
        vec![
            set("TZ", None, "Europe/Amsterdam"),
            set("LEMONFIBER_USENET", Some("off"), "on"),
            set("LEMONFIBER_TORRENT", None, "on"),
        ],
    );
}

#[test]
fn a_setting_that_was_present_but_empty_is_captured_as_empty_not_absent() {
    // An empty prior value is a value: rolling the write back must restore the
    // empty string, so `changes` records `Some("")`, distinct from the `None`
    // of a key that was never there.
    let mut wizard = on_native_linux();
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    let current = EnvFile::parse("LEMONFIBER_USENET=\n");
    let set = |key: &str, previous: Option<&str>, value: &str| Change {
        at: STAMPED,
        operation: "apply".to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: previous.map(str::to_owned),
            current: value.to_owned(),
        },
    };
    assert_eq!(
        wizard.plan().changes(&current, STAMPED),
        vec![
            set("TZ", None, "Europe/Amsterdam"),
            set("LEMONFIBER_USENET", Some(""), "on"),
            set("LEMONFIBER_TORRENT", None, "on"),
        ],
    );
}

#[test]
fn a_device_key_is_named_as_a_key_is() {
    let change = |kind| Change {
        at: Instant::EPOCH,
        operation: "device-key".to_owned(),
        target: "pixel".to_owned(),
        kind,
    };
    assert_eq!(
        described(&change(Kind::DeviceKeyRegistered {
            name: "pixel".to_owned(),
            backing: "tee".to_owned(),
        })),
        "the device key pixel"
    );
    assert_eq!(
        described(&change(Kind::DeviceKeyRevoked {
            name: "pixel".to_owned(),
        })),
        "the revoke of the device key pixel"
    );
    assert_eq!(
        described(&change(Kind::PushEndpointRegistered {
            name: "pixel".to_owned(),
        })),
        "the push endpoint pixel"
    );
    assert_eq!(
        described(&change(Kind::PushEndpointRemoved {
            name: "pixel".to_owned(),
        })),
        "the removal of the push endpoint pixel"
    );
    assert_eq!(
        described(&change(Kind::Overlaid {
            path: "/cfg/overlay.yml".to_owned(),
            previous: None,
            current: None,
        })),
        "the overlay /cfg/overlay.yml"
    );
    assert_eq!(
        described(&change(Kind::Certified {
            service: "jellyfin".to_owned(),
            previous: Some("a".to_owned()),
            current: "b".to_owned(),
        })),
        "the internal certificate of jellyfin"
    );
    assert_eq!(
        described(&change(Kind::Ran {
            service: "prowlarr".to_owned(),
            running: false,
        })),
        "the stop or start of prowlarr"
    );
    assert_eq!(
        described(&change(Kind::Restarted {
            service: "recyclarr".to_owned(),
        })),
        "the restart of recyclarr"
    );
    assert_eq!(
        described(&change(Kind::MovedInPlace {
            service: "decline".to_owned(),
            release: "0.18.0".to_owned(),
        })),
        "the move of decline onto its plugin"
    );
    assert_eq!(
        described(&change(Kind::Unqueued {
            kind: crate::ports::media::TitleKind::Tv,
            title: 9,
            release: "9f86d081".to_owned(),
        })),
        "the drop of a stuck download"
    );
    assert_eq!(
        described(&change(Kind::Asked {
            unit: crate::collected::Unit::Music(crate::ports::service::MusicUnit::Album),
            reference: "mbid-kind-of-blue".to_owned(),
            kept: "music-1".to_owned(),
            whole: true,
        })),
        "the ask music-1"
    );
}

#[test]
fn a_grant_is_named_by_its_plugin_and_credential() {
    assert_eq!(
        described(&Change {
            at: Instant::EPOCH,
            operation: "plugin:reader".to_owned(),
            target: "LIBRARY_CURATE_TV_API_KEY".to_owned(),
            kind: Kind::Granted {
                plugin: "reader".to_owned(),
                grant: crate::test_support::a_grant(),
                given: true,
            },
        }),
        "reader's grant of LIBRARY_CURATE_TV_API_KEY"
    );
}

#[test]
fn a_move_and_a_copy_are_said_plainly_enough_to_recognise() {
    let change = |kind| Change {
        at: Instant::EPOCH,
        operation: "project".to_owned(),
        target: "the mount tree".to_owned(),
        kind,
    };
    assert_eq!(
        described(&change(Kind::Moved {
            from: "/stack/config/jellyfin/cache".to_owned(),
            to: "/stack/mounts/jellyfin/cache".to_owned(),
        })),
        "the move of /stack/config/jellyfin/cache"
    );
    assert_eq!(
        described(&change(Kind::Copied {
            volume: "lemonfiber_caddy_data".to_owned(),
            to: "/stack/mounts/caddy/data".to_owned(),
        })),
        "the copy of the volume lemonfiber_caddy_data"
    );
}
