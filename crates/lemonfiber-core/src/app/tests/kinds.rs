//! Each outcome dispatching produces, written under its own kind.

use super::*;

/// A run that keeps archives, whose engine answers and reports nothing running.
fn keeping_archives(vault: &Arc<crate::app::fixtures::FakeArchive>) -> Ctx {
    let stopped = a_context()
        .engine(Arc::new(lemonfiber_fixtures::support::Reporting::holding(
            &["sonarr"],
            crate::ports::docker::Lifecycle::Exited,
            crate::ports::docker::Health::None,
        )))
        .settings(Settings {
            data_root: Some(std::path::PathBuf::from("/srv/media")),
            ..Settings::default()
        })
        .build();
    crate::app::fixtures::keeping(stopped, vault)
}

/// Where this copy stands, asked for here as well as from the integration test
/// beside it — the arm is reached from two compilations of this file and has to
/// run in both.
///
/// A machine that will not say where its own binary is, with the release list
/// switched off, which is the shortest route through the arm and the one that
/// shows nothing about it can refuse.
#[tokio::test]
async fn a_dispatched_update_check_serialises_under_its_own_kind() {
    let ctx = a_context()
        .settings(Settings {
            reaching: crate::config::Reaching::without(crate::config::REACH_UPDATES_KEY),
            ..Settings::default()
        })
        .build();
    let json = dispatch(Command::SelfUpdate { to: None }, &ctx)
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();

    assert!(json.contains(r#""kind":"self-update""#), "{json}");
    assert!(json.contains(r#""standing":"check-failed""#), "{json}");
    assert!(json.contains(r#""installed":"untellable""#), "{json}");
}

/// And the other half of the same word: naming a version asks about that one,
/// which is how going back is asked for.
#[tokio::test]
async fn naming_a_version_asks_about_that_one_and_says_what_it_reads() {
    let ctx = a_context()
        .settings(Settings {
            reaching: crate::config::Reaching::none(),
            ..Settings::default()
        })
        .build();
    let asked = dispatch(
        Command::SelfUpdate {
            to: Some("0.9.0".to_owned()),
        },
        &ctx,
    )
    .await;

    let named = matches!(
        &asked,
        Ok(Outcome::SelfUpdate(report))
            if report.asked.as_deref() == Some("0.9.0") && report.configuration.is_some()
    );
    assert!(named, "a named version was not asked about: {asked:?}");
}

/// What this machine keeps, asked for here as well as from the integration test
/// beside it — the arms are reached from two compilations of this file and have
/// to run in both.
#[tokio::test]
async fn a_dispatched_disclosure_serialises_under_its_own_kind() {
    let vault = Arc::new(crate::app::fixtures::FakeArchive::roomy());
    let json = dispatch(Command::Stored, &keeping_archives(&vault))
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();

    assert!(json.contains("\"kind\":\"stored\""), "{json}");
    assert!(json.contains("\"state\":\"not-asked\""), "{json}");
}

/// And the other half of the same answer. An unconfirmed removal lists what
/// would go and takes none of it, so this reaches the arm without a filesystem
/// being anywhere near it.
#[tokio::test]
async fn an_unconfirmed_removal_answers_with_what_would_go() {
    let vault = Arc::new(crate::app::fixtures::FakeArchive::roomy());
    let json = dispatch(
        Command::Forget { confirm: false },
        &keeping_archives(&vault),
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();

    assert!(json.contains("\"kind\":\"stored\""), "{json}");
    assert!(json.contains("\"state\":\"unconfirmed\""), "{json}");
}

#[tokio::test]
async fn a_dispatched_update_serialises_under_its_own_kind() {
    // Unconfirmed against an engine that has pulled nothing, so it answers with a
    // stack already on its pins and touches nothing — which is still the whole of
    // the dispatch, envelope and serialise arms.
    let ctx = a_context()
        .build()
        .with_images(lemonfiber_fixtures::pulled::Pulled::holding(Vec::new()));
    let json = dispatch(
        Command::Update(crate::update::run::Asked {
            service: None,
            confirm: false,
            wait: Waiting::Never,
        }),
        &ctx,
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();
    assert!(json.contains(r#""kind":"update""#), "{json}");
    assert!(json.contains(r#""state":"current""#), "{json}");
}

#[tokio::test]
async fn a_dispatched_backup_serialises_under_its_own_kind() {
    let vault = Arc::new(crate::app::fixtures::FakeArchive::roomy());
    let json = dispatch(Command::Backup { service: None }, &keeping_archives(&vault))
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
    assert!(json.contains(r#""kind":"backup""#), "{json}");
    assert!(json.contains("backups"), "it says where it went: {json}");
}

#[tokio::test]
async fn a_dispatched_restore_serialises_under_its_own_kind() {
    // Unconfirmed, so it lists what it would overwrite and touches nothing —
    // which is still the whole of the dispatch, envelope and serialise arms.
    let vault = Arc::new(crate::app::fixtures::FakeArchive::holding(
        crate::app::fixtures::CURRENT,
        crate::backup::SCHEMA,
    ));
    let json = dispatch(
        Command::Restore {
            archive: crate::app::restore::Kept::Named("lemonfiber-full-1.tar.gz".to_owned()),
            repoint: false,
            consent: crate::app::restore::Consent::List,
        },
        &keeping_archives(&vault),
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();
    assert!(json.contains(r#""kind":"restore""#), "{json}");
    assert!(
        json.contains(r#""done":null"#),
        "nothing was put back: {json}"
    );
}

#[tokio::test]
async fn a_dispatched_listing_serialises_under_its_own_kind() {
    // In-crate as well as from `tests/`, because this file carries tests of its
    // own and so is mapped twice: an arm reached only from outside the crate is
    // an arm the mapping this file's own tests build never runs.
    let vault = Arc::new(crate::app::fixtures::FakeArchive::keeping_backups(&[(
        "lemonfiber-full-1.tar.gz",
        "00000000000000000001",
    )]));
    let json = dispatch(Command::Archives, &keeping_archives(&vault))
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
    assert!(json.contains(r#""kind":"archives""#), "{json}");
    assert!(
        json.contains("lemonfiber-full-1.tar.gz"),
        "it names what is kept: {json}"
    );
}

#[tokio::test]
async fn a_dispatched_support_request_serialises_under_its_own_kind() {
    // Told to write nothing, so it describes a bundle and touches no disk —
    // which is still the whole of the dispatch, envelope and serialise arms.
    // In-crate as well as from `tests/`, because this file carries tests of its
    // own and so is mapped twice: an arm reached only from outside the crate is
    // an arm the mapping this file's own tests build never runs.
    let vault = Arc::new(crate::app::fixtures::FakeArchive::roomy());
    let ctx = keeping_archives(&vault).with_http(Fake::silent());
    let json = dispatch(
        Command::Support {
            write: false,
            wanted: super::super::bundle::Wanted::default(),
            dest: super::super::support::Destination::Kept,
        },
        &ctx,
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();
    assert!(json.contains(r#""kind":"bundle""#), "{json}");
    assert!(
        json.contains(r#""path":null"#),
        "nothing was written: {json}"
    );
}

#[tokio::test]
async fn a_dispatched_offer_serialises_under_its_own_kind() {
    // In-crate as well as from `tests/`, for the reason the bundle above is:
    // this file carries tests of its own and so is mapped twice, and an arm
    // reached only from outside the crate is one this mapping never runs.
    //
    // Given no consent, so it offers and puts none of it right — which is still
    // the whole of the dispatch, envelope and serialise arms.
    let json = dispatch(
        Command::Repair {
            consent: super::super::repair::Consent::Offer,
            disruptive: false,
        },
        &crate::app::fixtures::ctx_at("dispatched-offer"),
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();
    assert!(json.contains(r#""kind":"repair""#), "{json}");
    assert!(
        json.contains(r#""acted":false"#),
        "nothing was carried out: {json}"
    );
    // The offer names itself on the way out, which is what a consent sent back
    // in another request has to be able to say.
    assert!(json.contains(r#""agreement":"#), "{json}");
}

#[tokio::test]
async fn a_dispatched_reversal_serialises_under_its_own_kind() {
    let dir =
        std::env::temp_dir().join(format!("lemonfiber-dispatched-undo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("config"));
    let _ = std::fs::create_dir_all(dir.join("data"));
    let settings = Settings {
        env_file: Some(dir.join("config").join(".env")),
        stack_dir: Some(dir.join("data").join("stack")),
        ..Settings::default()
    };
    let json = dispatch(
        Command::Undo { run: None },
        &a_context().settings(settings).build(),
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();
    assert!(json.contains(r#""kind":"undo""#), "{json}");
    assert!(
        json.contains(r#""reversed":[]"#),
        "nothing had been repaired, so nothing went back: {json}"
    );
}

#[tokio::test]
async fn a_dispatched_quality_show_serialises_under_its_own_kind() {
    // Through dispatch, a quality command reaches its outcome, envelope and
    // serialisation — the arms the handler's own tests, calling it directly,
    // never touch. With no config the choice is the default, shown.
    let json = dispatch(Command::Quality(QualityAction::Show), &ctx(Ok(spoke(""))))
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
    assert!(
        json.contains("\"kind\":\"quality\""),
        "envelope names the kind"
    );
    assert!(json.contains("everything"), "the global choice is reported");
}

#[tokio::test]
async fn a_dispatched_quality_set_with_nowhere_to_record_is_an_error() {
    // The dispatch arm unboxes the handler's error: a set with no configured
    // env file has nowhere to write the choice, so it fails rather than lying.
    let refused = dispatch(
        Command::Quality(QualityAction::Set {
            preset: Preset::Balanced,
            media_type: None,
            confirm: false,
        }),
        &ctx(Ok(spoke(""))),
    )
    .await;
    assert!(
        refused.is_err(),
        "a set with nowhere to record cannot succeed"
    );
}

#[tokio::test]
async fn a_dispatched_trace_serialises_under_its_own_kind() {
    // No key opens a target, so no item matches and the trace stays offline while
    // exercising the dispatch, envelope and serialise arms for its outcome.
    let json = dispatch(
        Command::Trace {
            term: "the expanse".to_owned(),
            season: None,
            searching: false,
        },
        &ctx(Ok(spoke(""))),
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();
    assert!(
        json.contains("\"kind\":\"trace\""),
        "envelope names the kind"
    );
}

#[tokio::test]
async fn a_dispatched_household_serialises_under_its_own_kind() {
    // Nothing is recorded to sign in with, so the view reports itself unavailable —
    // which still exercises the dispatch, envelope and serialise arms for its outcome.
    let json = dispatch(Command::Household { member: None }, &ctx(Ok(spoke(""))))
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
    assert!(
        json.contains("\"kind\":\"household\""),
        "envelope names the kind"
    );
}

/// The shelf reports itself unread rather than empty where there is nothing to sign
/// in to the media server with — which is the distinction the whole report is built
/// around, and the one that would tell a household they own nothing on the day the
/// server rebooted.
#[tokio::test]
async fn a_shelf_with_no_media_server_behind_it_is_unread_not_empty() {
    let json = dispatch(
        Command::Held {
            member: "ada".to_owned(),
            most: 25,
        },
        &ctx(Ok(spoke(""))),
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();
    assert!(
        json.contains("\"kind\":\"held\""),
        "envelope names the kind"
    );
    assert!(
        json.contains("\"available\":false"),
        "an unread shelf reported itself as read: {json}"
    );
    assert!(
        json.contains("no recorded"),
        "an unread shelf did not say why: {json}"
    );
}

/// Both writes about what a household may ask for answer with the household.
///
/// Nothing is recorded to sign in with, so each refuses rather than writing — which
/// is the answer worth having here: the refusal names what was *not* changed, and a
/// command that reported a limit it had failed to set would be the worse of the two
/// ways to be wrong.
#[tokio::test]
async fn choosing_and_deciding_refuse_rather_than_claim_a_change() {
    let context = ctx(Ok(spoke("")));
    let chosen = dispatch(
        Command::Allowing(Chosen {
            member: None,
            policy: Some(crate::asking::Policy::Trusted),
            quota: None,
        }),
        &context,
    )
    .await;
    let decided = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::TurnedDown {
                reason: "no room this month".to_owned(),
            },
        }),
        &context,
    )
    .await;

    for refused in [chosen, decided] {
        let code = refused.err().map(|problem| problem.code);
        assert_eq!(code, Some(crate::asking::UNREACHABLE), "{code:?}");
    }
}

/// A refusal with no reason is refused before anything is reached.
///
/// It never gets as far as the service, which is the point: a blank reason is the
/// silent decline this exists to prevent, and it is stopped where it is written
/// rather than after somebody's request has already been turned down.
#[tokio::test]
async fn a_decline_with_a_blank_reason_never_reaches_the_service() {
    let refused = dispatch(
        Command::Deciding(Decision {
            request: 7,
            answer: Ruling::TurnedDown {
                reason: "   ".to_owned(),
            },
        }),
        &ctx(Ok(spoke(""))),
    )
    .await;

    assert_eq!(
        refused.err().map(|problem| problem.code),
        Some(crate::asking::NO_REASON)
    );
}

#[tokio::test]
async fn a_dispatched_front_door_serialises_under_its_own_kind() {
    // The stack this repository carries runs a request service, so the answer
    // names one — and the command runs through dispatch, envelope and serialise
    // for its outcome on the way.
    let household = a_context()
        .engine(Arc::new(Reporting::holding(
            &["seerr"],
            crate::ports::docker::Lifecycle::Running,
            crate::ports::docker::Health::Healthy,
        )))
        .build();
    let json = dispatch(Command::FrontDoor, &household)
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
    assert!(
        json.contains("\"kind\":\"front-door\""),
        "envelope names the kind"
    );
}

#[tokio::test]
async fn a_dispatched_stuck_serialises_under_its_own_kind() {
    // No key opens a target, so nothing is stuck — but the command still runs through
    // dispatch, envelope and serialise for its outcome.
    let json = dispatch(Command::Stuck, &ctx(Ok(spoke(""))))
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
    assert!(
        json.contains("\"kind\":\"stuck\""),
        "envelope names the kind"
    );
}

/// The whole of what leaves this machine, asked for here as well as from the
/// integration test beside it — the arm is reached from two compilations of this
/// file and has to run in both.
#[tokio::test]
async fn a_dispatched_enumeration_serialises_under_its_own_kind() {
    let json = dispatch(Command::Outbound, &ctx(Ok(spoke(""))))
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();

    assert!(json.contains("\"kind\":\"outbound\""), "{json}");
    assert!(json.contains("\"reach\":\"registry\""), "{json}");
}

/// Where the services come from, asked for here as well as from the integration
/// test beside it, for the reason the enumeration above is: the arm is reached
/// from two compilations of this file and has to run in both.
#[tokio::test]
async fn a_dispatched_listing_of_where_the_services_come_from_serialises_under_its_own_kind() {
    let json = dispatch(Command::Provenance, &ctx(Ok(spoke(""))))
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();

    assert!(json.contains("\"kind\":\"provenance\""), "{json}");
    assert!(json.contains("\"license\":"), "{json}");
    assert!(json.contains("\"upstream\":"), "{json}");
}

/// And the refusal, for the reason the enumeration's is: a stack that will not
/// read has nothing to say about what it bundles, and an empty listing would read
/// as a stack that bundles nothing.
#[tokio::test]
async fn a_listing_over_a_stack_that_will_not_read_is_refused() {
    let nowhere = a_context()
        .over(crate::test_support::nowhere())
        .runner(Arc::new(Scripted(Ok(spoke("")))))
        .engine(Arc::new(Reporting::default()))
        .build();

    assert!(dispatch(Command::Provenance, &nowhere).await.is_err());
}

/// And the refusal, because half an enumeration reads as the whole of it: a stack
/// that will not read cannot say what its services reach.
#[tokio::test]
async fn an_enumeration_over_a_stack_that_will_not_read_is_refused() {
    let nowhere = a_context()
        .over(crate::test_support::nowhere())
        .runner(Arc::new(Scripted(Ok(spoke("")))))
        .engine(Arc::new(Reporting::default()))
        .build();

    assert!(dispatch(Command::Outbound, &nowhere).await.is_err());
}

/// What the services are for, asked for here as well as from the integration test
/// beside it, for the reason the enumeration above is: the arm is reached from two
/// compilations of this file and has to run in both.
#[tokio::test]
async fn a_dispatched_catalogue_serialises_under_its_own_kind() {
    let json = dispatch(Command::Catalogue, &ctx(Ok(spoke(""))))
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();

    assert!(json.contains("\"kind\":\"catalogue\""), "{json}");
    assert!(json.contains("\"describes\":"), "{json}");
    assert!(json.contains("\"without_it\":"), "{json}");
    assert!(json.contains("\"removed\":"), "{json}");
}

/// And the refusal, for the reason the enumeration's is: a stack that will not
/// read has nothing to say about what it holds, and an empty listing would read as
/// a stack holding nothing.
#[tokio::test]
async fn a_catalogue_over_a_stack_that_will_not_read_is_refused() {
    let nowhere = a_context()
        .over(crate::test_support::nowhere())
        .runner(Arc::new(Scripted(Ok(spoke("")))))
        .engine(Arc::new(Reporting::default()))
        .build();

    assert!(dispatch(Command::Catalogue, &nowhere).await.is_err());
}

/// The words need no stack and no engine, so this is the one command that runs
/// through dispatch, envelope and serialise against a context that has nothing.
#[tokio::test]
async fn a_dispatched_explanation_serialises_under_its_own_kind() {
    let ctx = ctx(Ok(spoke("")));

    let word = dispatch(
        Command::Explain {
            word: "indexer".to_owned(),
        },
        &ctx,
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();
    assert!(word.contains("\"kind\":\"word\""), "{word}");
    assert!(word.contains("\"word\":\"indexer\""), "{word}");

    let listed = dispatch(Command::Glossary, &ctx)
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
    assert!(listed.contains("\"kind\":\"glossary\""), "{listed}");
    assert!(listed.contains("\"words\":[{"), "{listed}");
}

/// The record answers on a machine that has changed nothing, rather than refusing
/// for want of somewhere to keep a journal.
///
/// Dispatched here as well as beside the crate because the dispatcher is compiled
/// twice, and an arm exercised in only one copy has its coverage counted from the
/// other.
#[tokio::test]
async fn the_record_answers_on_a_machine_that_has_changed_nothing() {
    let ctx = ctx(Ok(spoke("")));

    let said = dispatch(Command::History, &ctx)
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();

    assert!(said.contains("\"kind\":\"history\""), "{said}");
    assert!(said.contains("\"changes\":[]"), "{said}");
    assert!(said.contains("\"horizon\":\""), "{said}");
}

/// Which app to watch on needs no stack and no engine either, for the same
/// reason: the client landscape belongs to the platforms rather than to this
/// machine, so it answers before anything is set up.
#[tokio::test]
async fn which_app_to_watch_on_answers_against_a_context_that_has_nothing() {
    let ctx = ctx(Ok(spoke("")));

    let said = dispatch(Command::Clients, &ctx)
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();

    assert!(said.contains("\"kind\":\"clients\""), "{said}");
    assert!(said.contains("\"devices\":[{"), "{said}");
    assert!(said.contains("\"only_at_home\""), "{said}");
}

/// Answering "it means nothing" for a word this product never explains would be a
/// wrong answer where an absent one was wanted.
#[tokio::test]
async fn a_word_with_no_entry_is_refused_rather_than_answered_emptily() {
    let refused = dispatch(
        Command::Explain {
            word: "kubernetes".to_owned(),
        },
        &ctx(Ok(spoke(""))),
    )
    .await
    .err()
    .map(|problem| problem.code.as_str().to_owned());

    assert_eq!(refused.as_deref(), Some("WORD-1"));
}

#[tokio::test]
async fn a_dispatched_setup_serialises_under_its_own_kind() {
    // Reading where the walk stands asks nothing and writes nothing, so it runs
    // through dispatch, envelope and serialise on a machine with no setup at all.
    let env_file = config_scratch("setup-kind");
    let settings = Settings {
        stack_dir: env_file.parent().map(|dir| dir.join("stack")),
        env_file: Some(env_file),
        ..Settings::default()
    };
    let json = dispatch(
        Command::Setup(SetupAction::Where),
        &a_context().settings(settings).build(),
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();
    assert!(
        json.contains("\"kind\":\"wizard\""),
        "envelope names the kind: {json}"
    );
    assert!(json.contains("\"at\":\"welcome\""), "{json}");
}

#[tokio::test]
async fn a_dispatched_reset_serialises_under_its_own_kind() {
    // The test stack is external, so a reset reverts nothing — but the command still
    // runs through dispatch, envelope and serialise for its outcome.
    let json = dispatch(Command::Reset { confirm: false }, &ctx(Ok(spoke(""))))
        .await
        .ok()
        .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
        .unwrap_or_default();
    assert!(
        json.contains("\"kind\":\"reset\""),
        "envelope names the kind"
    );
}

#[tokio::test]
async fn a_dispatched_music_choice_serialises_under_its_own_kind() {
    // A rehearsal records nothing and reaches no service, so it stays offline while
    // exercising the dispatch, envelope and serialise arms for its outcome.
    let mut context = ctx(Ok(spoke("")));
    context.dry_run = true;
    let json = dispatch(
        Command::QualityMusic {
            format: crate::audio::Format::Lossless,
        },
        &context,
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();
    assert!(
        json.contains("\"kind\":\"music\""),
        "envelope names the kind"
    );
}

#[tokio::test]
async fn a_dispatched_quality_upgrade_serialises_under_its_own_kind() {
    // Unconfirmed, it states the cost and reaches no service, so it stays offline
    // while exercising the dispatch, envelope and serialise arms for its outcome.
    let json = dispatch(
        Command::QualityUpgrade { confirm: false },
        &ctx(Ok(spoke(""))),
    )
    .await
    .ok()
    .map(|outcome| outcome.envelope().to_json().unwrap_or_default())
    .unwrap_or_default();
    assert!(
        json.contains("\"kind\":\"upgrade\""),
        "envelope names the kind"
    );
}

#[tokio::test]
async fn a_dispatched_accounting_of_the_disk_serialises_under_its_own_kind() {
    // A machine with no data location has nothing to account for, which is the
    // one answer this reaches with nothing running and nothing on a disk — and
    // it exercises the dispatch arm, which is what this is here for.
    let refused = dispatch(Command::Space { confirm: false }, &ctx(Ok(spoke(""))))
        .await
        .err()
        .map(|problem| problem.code);
    assert_eq!(refused, Some(crate::space::NOWHERE_TO_MEASURE));
}

#[tokio::test]
async fn a_dispatched_request_to_stop_seeding_reaches_its_own_arm() {
    // The same machine and the same refusal: a consequence nobody could state is
    // not one anybody can agree to, so a run that cannot account for the disk is
    // refused before it looks at what a client is holding. Which is also what
    // exercises the arm, and it is a different arm from the account beside it.
    let refused = dispatch(
        Command::StopSeeding {
            download: "A.Show.S01E01".to_owned(),
            agreement: None,
        },
        &ctx(Ok(spoke(""))),
    )
    .await
    .err()
    .map(|problem| problem.code);
    assert_eq!(refused, Some(crate::space::NOWHERE_TO_MEASURE));
}

#[tokio::test]
async fn a_dispatched_upgrade_over_an_unreadable_stack_is_an_error() {
    // The dispatch arm unboxes the driver's error: a confirmed upgrade cannot read
    // an unreadable stack's services, so it fails rather than half-acting.
    let ctx = a_context()
        .engine(Arc::new(Reporting::default()))
        .over(nowhere())
        .build();
    assert!(dispatch(Command::QualityUpgrade { confirm: true }, &ctx)
        .await
        .is_err());
}
