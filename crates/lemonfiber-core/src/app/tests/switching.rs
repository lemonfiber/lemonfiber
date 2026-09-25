//! Switching from one set of forms to another.

use super::*;

/// A rehearsing context whose engine can be reached and reports the named services
/// up. Reachable matters here in a way it does not for the other lifecycle
/// commands: a switch decides what to move by asking what is running, so an engine
/// that refuses the question stops it before it has anything to say.
fn switching(up: &[&str]) -> Ctx {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    a_context()
        .engine(Arc::new(Reporting::holding(
            up,
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(settings)
        .build()
        .rehearsing()
}

#[tokio::test]
async fn a_switch_onto_a_cold_stack_starts_the_closure_and_stops_nothing() {
    let ctx = switching(&[]);
    let switched = report(
        dispatch(
            Command::Switch {
                forms: vec!["tv".to_owned()],
            },
            &ctx,
        )
        .await,
    )
    .and_then(|report| report.switched);
    assert!(
        switched
            .as_ref()
            .is_some_and(|moved| moved.stopped.is_empty()
                && moved.kept.is_empty()
                && !moved.started.is_empty()
                && moved.stop_command.is_none()),
        "nothing is up, so there is nothing to stop and no command to stop it with: \
         {switched:?}"
    );
}

#[tokio::test]
async fn a_switch_stops_what_left_the_closure_and_keeps_what_stayed() {
    // Both are up; `library` holds the one and not the other. That difference is
    // the whole of narrowing, so it is asserted through dispatch rather than only
    // against the function that decides it.
    let ctx = switching(&["jellyfin", "qbittorrent"]);

    let switched = report(
        dispatch(
            Command::Switch {
                forms: vec!["library".to_owned()],
            },
            &ctx,
        )
        .await,
    )
    .and_then(|report| report.switched);

    assert_eq!(
        switched.as_ref().map(|moved| moved.stopped.clone()),
        Some(vec!["qbittorrent".to_owned()]),
        "only what fell outside the new closure is stopped: {switched:?}"
    );
    assert!(
        switched
            .as_ref()
            .is_some_and(|moved| moved.kept.contains(&"jellyfin".to_owned())),
        "and what both shapes hold keeps running: {switched:?}"
    );
    assert!(
        switched
            .as_ref()
            .is_some_and(|moved| moved
                .stop_command
                .as_ref()
                .is_some_and(|command| command.contains(&"--profile".to_owned())
                    && command.contains(&"torrent".to_owned())
                    && command.contains(&"stop".to_owned()))),
        "the stop names the profile it is leaving, or Compose will not accept the \
         service: {switched:?}"
    );
}

/// The whole of it, for real rather than rehearsed: the stop runs, the start runs
/// after it, and the switch waits for what it started the way starting a form
/// does. Everything `library` holds is reported up, so the wait has nothing to
/// wait for and the run reaches its own end rather than the patience deadline.
#[tokio::test]
async fn a_switch_that_succeeds_stops_starts_and_then_waits_for_health() {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .runner(Arc::new(Scripted(Ok(spoke("")))))
        .engine(Arc::new(Reporting::holding(
            &[
                "jellyfin",
                "seerr",
                "calibre-web-automated",
                "audiobookshelf",
                "navidrome",
                "qbittorrent",
            ],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(settings)
        .build();

    let report = report(
        dispatch(
            Command::Switch {
                forms: vec!["library".to_owned()],
            },
            &ctx,
        )
        .await,
    );

    assert_eq!(
        report.as_ref().and_then(|report| report.status),
        Some(0),
        "the status reported is the start's, not the stop's: {report:?}"
    );
    assert!(
        report
            .as_ref()
            .is_some_and(|report| report.condition.is_some() && !report.services.is_empty()),
        "and it waited, so it can say what the form came to: {report:?}"
    );
    assert!(
        report.as_ref().is_some_and(|report| report
            .switched
            .as_ref()
            .is_some_and(|moved| moved.stopped == vec!["qbittorrent".to_owned()])),
        "{report:?}"
    );
}

/// A context whose Compose cannot be run at all, holding the named services up.
///
/// Distinct from a Compose that ran and refused: one is a stack that said no, the
/// other is a machine with no Compose on it, and an operator can only act on the
/// second by installing something.
fn without_compose(up: &[&str]) -> Ctx {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    a_context()
        .runner(Arc::new(Scripted(Err(Failure::NotFound {
            program: "docker".to_owned(),
        }))))
        .engine(Arc::new(Reporting::holding(
            up,
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(settings)
        .build()
}

/// Both invocations a switch makes can fail to run rather than fail to work, and
/// the two arrive by different paths — the first while stopping what fell outside,
/// the second while starting what the new shape holds.
#[tokio::test]
async fn a_switch_that_cannot_run_the_stop_says_so() {
    let refusal = dispatch(
        Command::Switch {
            forms: vec!["library".to_owned()],
        },
        // Something is up and outside `library`, so the stop is the first thing run.
        &without_compose(&["qbittorrent"]),
    )
    .await
    .err()
    .map(|problem| problem.code);

    assert_eq!(refusal, Some(crate::error::codes::proc::MISSING_PROGRAM));
}

#[tokio::test]
async fn a_switch_that_cannot_run_the_start_says_so() {
    let refusal = dispatch(
        Command::Switch {
            forms: vec!["library".to_owned()],
        },
        // Nothing is up, so there is nothing to stop and the start is run first.
        &without_compose(&[]),
    )
    .await
    .err()
    .map(|problem| problem.code);

    assert_eq!(refusal, Some(crate::error::codes::proc::MISSING_PROGRAM));
}

/// A start that Compose refuses is reported as it stands rather than waited on:
/// there is nothing to wait for, and a run that said what the form came to would
/// be describing a form that never came to anything.
#[tokio::test]
async fn a_switch_whose_start_fails_reports_it_without_waiting() {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .runner(Arc::new(Scripted(Ok(refused("no such image")))))
        // Nothing is up, so nothing is stopped and the start is the only thing run.
        .engine(Arc::new(Reporting::holding(
            &[],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(settings)
        .build();

    let report = report(
        dispatch(
            Command::Switch {
                forms: vec!["library".to_owned()],
            },
            &ctx,
        )
        .await,
    );

    assert_eq!(report.as_ref().and_then(|report| report.status), Some(1));
    assert!(
        report
            .as_ref()
            .is_some_and(|report| report.condition.is_none() && report.services.is_empty()),
        "nothing started, so there is nothing it could have waited for: {report:?}"
    );
}

/// Starting the new set over one that would not go down is how two shapes of the
/// stack come to be running at once, so a stop that fails ends the switch there.
#[tokio::test]
async fn a_switch_whose_stop_fails_does_not_go_on_to_start_anything() {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .runner(Arc::new(Scripted(Ok(refused("that one is busy")))))
        .engine(Arc::new(Reporting::holding(
            &["qbittorrent"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(settings)
        .build();

    let report = report(
        dispatch(
            Command::Switch {
                forms: vec!["library".to_owned()],
            },
            &ctx,
        )
        .await,
    );

    assert_eq!(
        report.as_ref().and_then(|report| report.status),
        Some(1),
        "the failure the stop reported is what the switch reports"
    );
    assert!(
        report
            .as_ref()
            .is_some_and(|report| report.services.is_empty() && report.condition.is_none()),
        "and nothing was started, so there is nothing to have waited on: {report:?}"
    );
}

/// A switch works out what to move by asking the engine what is running, so one it
/// cannot reach leaves it with nothing to say — even rehearsing. Refusing is the
/// honest answer: reporting "would stop: nothing" would be a claim about a stack it
/// never managed to look at. This is the one lifecycle command a rehearsal cannot
/// answer without a daemon, and it is worth the difference.
#[tokio::test]
async fn a_switch_that_cannot_reach_the_engine_refuses_rather_than_guessing() {
    let refusal = dispatch(
        Command::Switch {
            forms: vec!["library".to_owned()],
        },
        &rehearsing(crate::config::Protocols::both()),
    )
    .await
    .err()
    .map(|problem| problem.code);

    assert_eq!(
        refusal,
        Some(crate::error::codes::docker::ENGINE_UNREACHABLE),
        "an engine that will not answer stops the switch rather than shrinking it"
    );
}

/// The published shape, pinned so a rename here breaks a test rather than a script.
#[tokio::test]
async fn a_switch_publishes_what_it_moved_under_the_names_a_script_reads() {
    let ctx = switching(&["jellyfin"]);
    let json = report(
        dispatch(
            Command::Switch {
                forms: vec!["library".to_owned()],
            },
            &ctx,
        )
        .await,
    )
    .and_then(|report| serde_json::to_string(&report).ok());

    assert!(
        json.as_ref()
            .is_some_and(|json| json.contains("\"switched\"")
                && json.contains("\"stopped\"")
                && json.contains("\"started\"")
                && json.contains("\"kept\"")),
        "{json:?}"
    );
}
