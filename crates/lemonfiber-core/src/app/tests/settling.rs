//! Waiting for services to become usable, and stopping them.

use super::*;

#[tokio::test]
async fn starting_waits_until_the_services_are_usable() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let ctx = watching(engine);
    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };

    let produced = report(dispatch(command, &ctx).await);
    assert_eq!(
        produced.map(|report| (
            report.condition,
            report.services.len(),
            report
                .services
                .iter()
                .all(|service| service.state == ServiceState::Healthy)
        )),
        Some((Some(Condition::Active), LIBRARY.len(), true)),
        "started means every service answered, not that a process exists"
    );
}

#[tokio::test]
async fn starting_keeps_asking_until_the_services_are_ready() {
    // Unsettled on the first two listings and healthy on the third, which
    // is what a stack that is genuinely starting looks like. A gate that
    // only ever read the engine once would pass this test by luck and fail
    // every real start.
    let engine =
        Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting).settling_after(2);
    let ctx = watching(engine);
    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };

    let produced = report(dispatch(command, &ctx).await);
    assert_eq!(
        produced.map(|report| report.condition),
        Some(Some(Condition::Active)),
        "waiting is the point: the answer changed while it waited"
    );
}

/// The requirement stated directly, and the only way to state it: "nothing was
/// torn down" is a claim about commands that were never issued, so it is asserted
/// against everything the runner was handed rather than against what came back.
#[tokio::test]
async fn one_service_failing_to_start_never_takes_down_the_rest() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .runner(runner.clone())
        .engine(Arc::new(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Starting,
        )))
        .settings(settings)
        .build()
        .with_http(Fake::scripted(Vec::new()))
        .with_patience(Duration::ZERO);

    let refused = dispatch(
        Command::Up {
            forms: vec!["library".to_owned()],
        },
        &ctx,
    )
    .await
    .err();

    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(super::super::NEVER_SETTLED),
        "the start is reported as not having finished"
    );
    assert!(
        runner.ran("up"),
        "it did try to start the form, so the claim below is about a real run"
    );
    assert!(
        !runner.ran("down"),
        "and never tore it down again for the one service that would not settle"
    );
    assert!(!runner.ran("stop"), "nor stopped what had already started");
}

/// A report about a container is a report about the wrong thing. What the operator
/// lost is what the stack says the service was there to do.
#[tokio::test]
async fn a_service_that_will_not_start_says_what_its_absence_costs() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting);
    let refused = dispatch(
        Command::Up {
            forms: vec!["library".to_owned()],
        },
        &watching(engine).with_patience(Duration::ZERO),
    )
    .await
    .err();

    assert_eq!(
        refused.as_ref().map(|problem| problem
            .meaning
            .contains("Files on disk, no way to watch them")),
        Some(true),
        "the manifest's own words for what jellyfin is for: {refused:?}"
    );
    assert_eq!(
        refused
            .as_ref()
            .map(|problem| problem.meaning.contains("left alone")),
        Some(true),
        "and the operator is told the rest of the form was not taken down with it"
    );
}

#[tokio::test]
async fn a_service_that_never_becomes_usable_stops_the_start_and_says_which() {
    // A container that is running but still inside its start period is
    // exactly the case a process check would have called success.
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting)
        .saying("jellyfin", "Cannot open database, disk is full");
    let ctx = watching(engine).with_patience(Duration::ZERO);
    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };

    let refused = dispatch(command, &ctx).await.err();
    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(super::super::NEVER_SETTLED)
    );
    assert_eq!(
        refused
            .as_ref()
            .map(|problem| problem.summary.contains("jellyfin")),
        Some(true),
        "the operator is told which service, not that something went wrong"
    );
    assert_eq!(
        refused
            .and_then(|problem| problem.detail)
            .map(|detail| detail.contains("disk is full")),
        Some(true),
        "the explanation is already on screen rather than left to be found"
    );
}

#[tokio::test]
async fn a_service_that_will_not_start_and_says_nothing_still_reports_which() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting);
    let ctx = watching(engine).with_patience(Duration::ZERO);
    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };

    let refused = dispatch(command, &ctx).await.err();
    assert_eq!(
        refused.map(|problem| (problem.code, problem.detail)),
        Some((super::super::NEVER_SETTLED, None)),
        "silence is reported as silence rather than as an empty quotation"
    );
}

#[tokio::test]
async fn a_crash_loop_is_not_something_starting_waits_out() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Restarting, Health::None);
    let ctx = watching(engine).with_patience(Duration::from_secs(3600));
    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };

    // Patience of an hour, and this must still return at once: a loop has
    // settled, and waiting for it is waiting forever.
    let produced = report(dispatch(command, &ctx).await);
    assert_eq!(
        produced.map(|report| report.condition),
        Some(Some(Condition::Degraded))
    );
}

/// The requirement: an operator who started two overlapping forms and stops one
/// of them is not asking for the other to lose its services. The refusal names
/// the form, because one told only "cannot stop" cannot act on it.
#[tokio::test]
async fn stopping_a_form_another_running_one_needs_is_refused_by_name() {
    // Everything `tv` and `movies` both hold, plus the one service each has of its
    // own — so both are up in their own right and neither contains the other.
    let running = Reporting::holding(
        &[
            "flaresolverr",
            "nzbhydra2",
            "prowlarr",
            "sabnzbd",
            "gluetun",
            "qbittorrent",
            "bazarr",
            "sonarr",
            "radarr",
        ],
        Lifecycle::Running,
        Health::Healthy,
    );
    let refused = dispatch(
        Command::Down {
            forms: vec!["tv".to_owned()],
            wait: Waiting::Never,
        },
        &watching(running),
    )
    .await
    .err();

    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(super::super::STILL_NEEDED)
    );
    assert!(
        refused
            .as_ref()
            .is_some_and(|problem| problem.summary.contains("movies")),
        "the operator is told which form still needs it: {refused:?}"
    );
}

/// Stopping asks what else is running before it acts, so an engine that will not
/// answer stops the stop. It cannot be overruled into taking down something it was
/// never able to see — and this is the one lifecycle command where that is a new
/// requirement, which makes it worth saying plainly rather than discovering.
#[tokio::test]
async fn stopping_reports_an_engine_it_cannot_see() {
    let refusal = dispatch(
        Command::Down {
            forms: vec!["library".to_owned()],
            wait: Waiting::Never,
        },
        &rehearsing(crate::config::Protocols::both()),
    )
    .await
    .err()
    .map(|problem| problem.code);

    assert_eq!(
        refusal,
        Some(crate::error::codes::docker::ENGINE_UNREACHABLE)
    );
}

/// The ordinary case, and the one that must not be made harder: one form up, that
/// form stopped, nothing else running to be deprived of anything.
#[tokio::test]
async fn stopping_the_only_form_that_is_up_is_not_refused() {
    let running = Reporting::holding(
        &[
            "jellyfin",
            "seerr",
            "calibre-web-automated",
            "audiobookshelf",
        ],
        Lifecycle::Running,
        Health::Healthy,
    );
    let produced = report(
        dispatch(
            Command::Down {
                forms: vec!["library".to_owned()],
                wait: Waiting::Never,
            },
            &watching(running),
        )
        .await,
    );

    assert_eq!(
        produced.map(|report| report.action),
        Some("down".to_owned()),
        "nothing else holds what `library` holds, so it simply stops"
    );
}

#[tokio::test]
async fn stopping_does_not_wait_for_anything() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Starting);
    let ctx = watching(engine).with_patience(Duration::ZERO);
    let command = Command::Down {
        forms: vec!["library".to_owned()],
        wait: Waiting::Never,
    };

    let produced = report(dispatch(command, &ctx).await);
    assert_eq!(
        produced.map(|report| (report.condition, report.services.is_empty())),
        Some((None, true)),
        "stopping is finished when Compose says so"
    );
}

/// Stopping named services is a different request from tearing a form down, and
/// it reaches Compose as a different word — `stop`, which leaves them where they
/// are, rather than `down`, which removes what the form started.
#[tokio::test]
async fn stopping_named_services_stops_only_those() {
    let engine = Reporting::holding(&LIBRARY, Lifecycle::Running, Health::Healthy);
    let ctx = watching(engine).with_patience(Duration::ZERO);
    let command = Command::Halt {
        forms: vec!["library".to_owned()],
        services: vec!["sonarr".to_owned()],
    };

    let produced = report(dispatch(command, &ctx).await);
    assert_eq!(
        produced
            .as_ref()
            .map(|report| report.action.clone())
            .as_deref(),
        Some("stop")
    );
    assert!(
        produced.is_some_and(|report| report.command.ends_with(&[
            "stop".to_owned(),
            "--".to_owned(),
            "sonarr".to_owned()
        ])),
        "the named service is fenced off from option parsing"
    );
}

#[tokio::test]
async fn a_compose_invocation_that_failed_is_not_then_waited_on() {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .runner(Arc::new(Scripted(Ok(refused("no such image")))))
        .engine(Arc::new(Reporting::holding(
            &LIBRARY,
            Lifecycle::Running,
            Health::Starting,
        )))
        .settings(settings)
        .build()
        .with_patience(Duration::ZERO);

    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };
    let produced = report(dispatch(command, &ctx).await);
    assert_eq!(
        produced.map(|report| (report.status, report.condition)),
        Some((Some(1), None)),
        "waiting for health after Compose refused would report the wrong fault"
    );
}

#[tokio::test]
async fn starting_reports_an_engine_it_cannot_see() {
    let ctx = watching(Reporting::absent());
    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };
    assert_eq!(
        dispatch(command, &ctx).await.err().map(|p| p.code),
        Some(crate::error::codes::docker::ENGINE_UNREACHABLE)
    );
}
