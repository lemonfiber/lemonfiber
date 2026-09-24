//! What a reviewed setup plans, applies and puts back.

use super::*;

#[test]
fn a_reviewed_wizard_plans_every_setting_it_gathered() {
    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "LEMONFIBER_USENET"), Some("on"));
    assert_eq!(setting(&plan, "LEMONFIBER_TORRENT"), Some("on"));
    assert_eq!(setting(&plan, "DATA_ROOT"), Some("/srv/media"));
    assert_eq!(setting(&plan, "PUID"), Some("1000"));
    // Distinct from PUID, so a swapped mapping would not pass unnoticed.
    assert_eq!(setting(&plan, "PGID"), Some("1001"));
    assert_eq!(setting(&plan, "JELLYFIN_MODE"), Some("docker"));
}

/// Every answer setup writes has a row saying what changing it costs.
///
/// What holds the catalogue to the wizard. A setting setup starts writing without
/// one is a decision the operator can make and cannot be told the price of, which
/// is the trap A4 exists to close — and it would be closed silently, since nothing
/// else notices a missing row.
#[test]
fn every_answer_setup_writes_is_catalogued_as_a_decision() {
    // Not decisions: nobody chooses these. They record what proving the credential
    // came to, so there is no cost to state for changing one.
    const RECORDED_RATHER_THAN_CHOSEN: [&str; 2] = [
        crate::config::INDEXER_VALIDATED_KEY,
        crate::config::PROVIDER_VALIDATED_KEY,
    ];

    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    wizard
        .answer(Answer::Credentials(Some(super::super::Indexer {
            url: "http://indexer.test/api".to_owned(),
            key: "the-key".to_owned(),
            validated: true,
        })))
        .unwrap_or(());
    wizard
        .answer(Answer::Provider(Some(super::super::Provider {
            host: "news.provider.test".to_owned(),
            port: 563,
            user: "person".to_owned(),
            pass: "the-login".to_owned(),
            tls: true,
            validated: true,
        })))
        .unwrap_or(());

    let plan = wizard.plan();
    let unpriced: Vec<&str> = plan
        .settings()
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| !RECORDED_RATHER_THAN_CHOSEN.contains(name))
        .filter(|name| crate::reconfigure::decision(name).is_none())
        .collect();
    assert!(
        unpriced.is_empty(),
        "setup writes these and nothing says what changing them costs: {unpriced:?}"
    );
    // An empty plan would pass the filter above without proving anything.
    assert!(
        plan.settings().len() > 10,
        "a plan this small is not exercising the writer"
    );
}

#[test]
fn every_setting_a_plan_writes_is_one_lemonfiber_declares() {
    // What holds the writer to `config::SETTINGS`. That list is what the guard on
    // withholding reads to know which settings exist at all, and a key written here
    // and absent there is a setting served to a browser that no guard has looked at.
    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    wizard
        .answer(Answer::Credentials(Some(super::super::Indexer {
            url: "http://indexer.test/api".to_owned(),
            key: "the-key".to_owned(),
            validated: true,
        })))
        .unwrap_or(());
    wizard
        .answer(Answer::Provider(Some(super::super::Provider {
            host: "news.provider.test".to_owned(),
            port: 563,
            user: "person".to_owned(),
            pass: "the-login".to_owned(),
            tls: true,
            validated: true,
        })))
        .unwrap_or(());

    let plan = wizard.plan();
    let undeclared: Vec<&str> = plan
        .settings()
        .iter()
        .map(|(name, _)| name.as_str())
        .filter(|name| !crate::config::SETTINGS.contains(name))
        .collect();
    assert!(
        undeclared.is_empty(),
        "setup writes these and config::SETTINGS does not name them, so nothing \
         checks whether they are displayed: {undeclared:?}"
    );
    assert!(
        plan.settings().len() > 10,
        "a plan this small is not exercising the writer"
    );
}

#[test]
fn every_setting_a_step_names_is_one_the_plan_it_belongs_to_writes() {
    // The other end of the same loop. `Step::settings` is what reconfiguration
    // reads to say which credentials adding a protocol opens, and a key named
    // there that setup never writes would send an operator looking for a question
    // this product does not ask.
    let mut wizard = on_native_linux();
    answer_all(&mut wizard);
    wizard
        .answer(Answer::Credentials(Some(super::super::Indexer {
            url: "http://indexer.test/api".to_owned(),
            key: "the-key".to_owned(),
            validated: true,
        })))
        .unwrap_or(());
    wizard
        .answer(Answer::Provider(Some(super::super::Provider {
            host: "news.provider.test".to_owned(),
            port: 563,
            user: "person".to_owned(),
            pass: "the-login".to_owned(),
            tls: true,
            validated: true,
        })))
        .unwrap_or(());

    let plan = wizard.plan();
    let written: Vec<&str> = plan
        .settings()
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    let named: Vec<&str> = Step::ORDER
        .into_iter()
        .flat_map(|step| step.settings().iter().copied())
        .collect();
    assert!(named.len() > 10, "the steps name too little to be a loop");
    let unwritten: Vec<&&str> = named.iter().filter(|key| !written.contains(key)).collect();
    assert!(
        unwritten.is_empty(),
        "these steps name settings setup never writes: {unwritten:?}"
    );
}

#[test]
fn a_given_indexer_is_planned_but_a_stale_one_is_dropped_when_it_no_longer_applies() {
    let indexer = Answer::Credentials(Some(super::super::Indexer {
        url: "http://indexer.test/api".to_owned(),
        key: "the-key".to_owned(),
        validated: true,
    }));

    // Chosen with a download protocol, the indexer is written.
    let mut wizard = on_native_linux();
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    wizard.answer(indexer.clone()).unwrap_or(());
    let plan = wizard.plan();
    assert_eq!(
        setting(&plan, "INDEXER_URL"),
        Some("http://indexer.test/api")
    );
    assert_eq!(setting(&plan, "INDEXER_APIKEY"), Some("the-key"));
    assert_eq!(setting(&plan, "INDEXER_VALIDATED"), Some("on"));

    // Then neither protocol is chosen, so the step no longer applies. The
    // answer lingers, but its key must not be written for a stack with no
    // service to use it.
    wizard
        .answer(Answer::Protocols(Protocols::none()))
        .unwrap_or(());
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "INDEXER_URL"), None);
    assert_eq!(
        setting(&plan, "INDEXER_APIKEY"),
        None,
        "no stale key is written"
    );
}

#[test]
fn a_given_provider_is_planned_but_dropped_when_usenet_is_no_longer_chosen() {
    let provider = Answer::Provider(Some(super::super::Provider {
        host: "news.provider.test".to_owned(),
        port: 563,
        user: "person".to_owned(),
        pass: "secret".to_owned(),
        tls: true,
        validated: true,
    }));

    // Chosen with Usenet, the provider login is written.
    let mut wizard = on_native_linux();
    wizard
        .answer(Answer::Protocols(Protocols::both()))
        .unwrap_or(());
    wizard.answer(provider.clone()).unwrap_or(());
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "USENET_HOST"), Some("news.provider.test"));
    assert_eq!(setting(&plan, "USENET_PORT"), Some("563"));
    assert_eq!(setting(&plan, "USENET_USER"), Some("person"));
    assert_eq!(setting(&plan, "USENET_VALIDATED"), Some("on"));

    // Then torrents only, so the provider step no longer applies; its login,
    // password and all, must not be written for a stack that will not use it.
    wizard
        .answer(Answer::Protocols(Protocols {
            usenet: false,
            torrent: true,
        }))
        .unwrap_or(());
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "USENET_HOST"), None);
    assert_eq!(
        setting(&plan, "USENET_PASS"),
        None,
        "no stale password is written"
    );
}

#[test]
fn an_unanswered_question_contributes_no_setting() {
    // A fresh wizard writes nothing; a partly answered one writes only what it
    // has, never a guessed default for what it does not.
    assert!(on_native_linux().plan().settings().is_empty());

    let mut wizard = on_native_linux();
    wizard
        .answer(Answer::Protocols(Protocols {
            usenet: true,
            torrent: false,
        }))
        .unwrap_or(());
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "LEMONFIBER_USENET"), Some("on"));
    // A declined protocol is written off, not omitted.
    assert_eq!(setting(&plan, "LEMONFIBER_TORRENT"), Some("off"));
    assert_eq!(setting(&plan, "DATA_ROOT"), None);
    assert_eq!(setting(&plan, "JELLYFIN_MODE"), None);
}

#[test]
fn a_declined_container_user_writes_no_ids() {
    let mut wizard = on_native_linux();
    wizard.answer(Answer::ServiceUser(None)).unwrap_or(());
    // The step is answered — recorded as "no id" rather than left open — and
    // that answered-with-nothing still writes neither id.
    assert_eq!(wizard.answers().service_user, Some(None));
    let plan = wizard.plan();
    assert_eq!(setting(&plan, "PUID"), None);
    assert_eq!(setting(&plan, "PGID"), None);
}

#[test]
fn the_library_choice_maps_to_its_mode_or_to_nothing() {
    let mut docker = on_native_linux();
    docker
        .answer(Answer::Library(Library::JellyfinDocker))
        .unwrap_or(());
    assert_eq!(setting(&docker.plan(), "JELLYFIN_MODE"), Some("docker"));

    let mut native = on_macos();
    native
        .answer(Answer::Library(Library::JellyfinNative))
        .unwrap_or(());
    assert_eq!(setting(&native.plan(), "JELLYFIN_MODE"), Some("native"));

    let mut none = on_native_linux();
    none.answer(Answer::Library(Library::None)).unwrap_or(());
    assert_eq!(setting(&none.plan(), "JELLYFIN_MODE"), None);
}

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
fn a_written_change_is_said_plainly_enough_to_recognise() {
    // What whoever is choosing is shown of an interrupted run: the point is
    // that they recognise it, not that it round-trips.
    let change = |kind| Change {
        at: String::new(),
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
        at: "t".to_owned(),
        operation: "apply".to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: previous.map(str::to_owned),
            current: value.to_owned(),
        },
    };
    assert_eq!(
        wizard.plan().changes(&current, "t"),
        vec![
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
        at: "t".to_owned(),
        operation: "apply".to_owned(),
        target: ".env".to_owned(),
        kind: Kind::Set {
            key: key.to_owned(),
            previous: previous.map(str::to_owned),
            current: value.to_owned(),
        },
    };
    assert_eq!(
        wizard.plan().changes(&current, "t"),
        vec![
            set("LEMONFIBER_USENET", Some(""), "on"),
            set("LEMONFIBER_TORRENT", None, "on"),
        ],
    );
}
