//! Starting, stopping and pulling, as the engine is asked to.

use super::*;

#[tokio::test]
async fn starting_a_form_reports_what_it_would_run_and_runs_nothing() {
    let ctx = rehearsing(crate::config::Protocols::both());
    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };
    let produced = report(dispatch(command, &ctx).await);
    assert_eq!(
        produced.map(|report| (
            report.action,
            report.plan.profiles.into_iter().collect::<Vec<String>>(),
            report.rehearsed,
            report.status,
            report.command.last().cloned()
        )),
        Some((
            "up".to_owned(),
            vec!["media".to_owned()],
            true,
            None,
            Some("--detach".to_owned())
        )),
        "a rehearsal reports the command and never ran it"
    );
}

#[tokio::test]
async fn what_the_configuration_left_out_is_reported_rather_than_dropped() {
    let ctx = rehearsing(crate::config::Protocols {
        usenet: true,
        torrent: false,
    });
    let command = Command::Up {
        forms: vec!["tv".to_owned()],
    };
    let produced = report(dispatch(command, &ctx).await);

    assert_eq!(
        produced.map(|report| report.plan.dropped),
        Some(vec![crate::stack::closure::Dropped {
            profile: "torrent".to_owned(),
            needs: lemonfiber_manifest::Protocol::Torrent,
        }]),
        "the operator hears which service is missing, and why"
    );
}

#[tokio::test]
async fn a_real_run_reports_how_the_command_exited() {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    // Reachable and holding nothing: stopping asks what else is running before it
    // acts, and an engine that will not answer is a refusal rather than a run.
    let ctx = a_context()
        .engine(Arc::new(Reporting::holding(
            &[],
            Lifecycle::Exited,
            Health::None,
        )))
        .settings(settings)
        .build();
    let command = Command::Down {
        forms: vec!["library".to_owned()],
        wait: Waiting::Never,
    };
    let produced = report(dispatch(command, &ctx).await);

    assert_eq!(
        produced.map(|report| (report.action, report.rehearsed, report.status)),
        Some(("down".to_owned(), false, Some(0)))
    );
}

#[tokio::test]
async fn starting_named_services_is_the_start_compose_spells_that_way() {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .engine(Arc::new(Reporting::holding(
            &["sabnzbd", "gluetun", "qbittorrent"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(settings)
        .build()
        .with_patience(std::time::Duration::ZERO);
    let command = Command::Start {
        forms: vec!["dl".to_owned()],
        services: vec!["qbittorrent".to_owned()],
    };
    let produced = report(dispatch(command, &ctx).await);

    assert_eq!(
        produced
            .as_ref()
            .map(|report| report.action.clone())
            .as_deref(),
        Some("up"),
        "it is a start, and reports as one"
    );
    assert!(
        produced.is_some_and(|report| report.command.contains(&"qbittorrent".to_owned())),
        "and the command it ran names the service rather than the form"
    );
}

#[tokio::test]
async fn a_teardown_asked_to_wait_where_nothing_is_downloading_stops_at_once() {
    // A form holding no download client asks the network nothing, so the wait
    // it was asked for is over before the teardown that follows it begins.
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .engine(Arc::new(Reporting::holding(
            &[],
            Lifecycle::Exited,
            Health::None,
        )))
        .settings(settings)
        .build();
    let command = Command::Down {
        forms: vec!["search".to_owned()],
        wait: Waiting::ForTheDownloads,
    };
    let produced = report(dispatch(command, &ctx).await);

    assert_eq!(
        produced.map(|report| (report.action, report.status)),
        Some(("down".to_owned(), Some(0)))
    );
}

#[tokio::test]
async fn an_engine_that_will_not_start_is_reported_to_the_operator() {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .runner(Arc::new(Scripted(Err(Failure::NotFound {
            program: "docker".to_owned(),
        }))))
        .engine(Arc::new(Reporting::default()))
        .settings(settings)
        .build();
    let command = Command::Pull {
        forms: vec!["library".to_owned()],
    };
    let refusal = dispatch(command, &ctx)
        .await
        .err()
        .map(|problem| problem.code);
    assert_eq!(refusal, Some(crate::error::codes::proc::MISSING_PROGRAM));
}

#[tokio::test]
async fn pull_progress_streams_composes_output_line_by_line_then_the_exit() {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .runner(Arc::new(Scripted(Ok(Output {
            status: Some(0),
            stdout: "library Pulling\nlibrary Pulled\n".to_owned(),
            stderr: String::new(),
        }))))
        .engine(Arc::new(Reporting::default()))
        .settings(settings)
        .build();

    let (closed, silent) = tokio::sync::mpsc::channel(1);
    drop(closed);
    let mut progress = pull_progress(&ctx, &["library".to_owned()])
        .await
        .unwrap_or(silent);
    let mut lines = Vec::new();
    let mut status = None;
    while let Some(event) = progress.recv().await {
        match event {
            Progress::Line(line) => lines.push(line),
            Progress::Ended(code) => status = code,
        }
    }
    // Each of Compose's per-image lines arrives on the stream, then the exit —
    // what a surface renders as it happens rather than after.
    assert_eq!(
        lines,
        vec!["library Pulling".to_owned(), "library Pulled".to_owned()]
    );
    assert_eq!(status, Some(0));
}

#[tokio::test]
async fn a_pull_that_cannot_spawn_compose_is_a_problem_not_a_stream() {
    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        ..Settings::default()
    };
    let ctx = a_context()
        .runner(Arc::new(Scripted(Err(Failure::NotFound {
            program: "docker".to_owned(),
        }))))
        .engine(Arc::new(Reporting::default()))
        .settings(settings)
        .build();
    let refusal = pull_progress(&ctx, &["library".to_owned()])
        .await
        .err()
        .map(|problem| problem.code);
    assert_eq!(refusal, Some(crate::error::codes::proc::MISSING_PROGRAM));
}

#[tokio::test]
async fn restarting_names_the_services_and_nothing_else() {
    let ctx = rehearsing(crate::config::Protocols::both());
    let command = Command::Restart {
        forms: vec!["library".to_owned()],
        services: vec!["jellyfin".to_owned()],
    };
    let produced = report(dispatch(command, &ctx).await);

    assert_eq!(
        produced.map(|report| (report.action, report.command.last().cloned())),
        Some(("restart".to_owned(), Some("jellyfin".to_owned())))
    );
}

#[tokio::test]
async fn a_form_this_stack_does_not_have_never_reaches_the_engine() {
    let ctx = rehearsing(crate::config::Protocols::both());
    let command = Command::Up {
        forms: vec!["telly".to_owned()],
    };
    let outcome = dispatch(command, &ctx).await;
    assert_eq!(
        outcome.as_ref().err().map(|problem| problem.code),
        Some(crate::error::codes::form::NO_SUCH_FORM)
    );
    assert_eq!(report(outcome), None, "nothing ran, so there is no report");
}

#[tokio::test]
async fn an_unreadable_stack_is_reported_before_anything_is_started() {
    let nowhere = Source::External(std::path::Path::new("/lemonfiber/no/such/stack"));
    let ctx = a_context()
        .engine(Arc::new(Reporting::default()))
        .over(nowhere)
        .build();
    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };
    assert_eq!(
        dispatch(command, &ctx).await.err().map(|p| p.code),
        Some(crate::error::codes::stack::STACK_UNREADABLE)
    );
}

#[tokio::test]
async fn an_embedded_stack_with_nowhere_to_go_stops_before_starting_anything() {
    static EMBEDDED: include_dir::Dir<'_> =
        include_dir::include_dir!("$CARGO_MANIFEST_DIR/../../assets/media-stack");

    let settings = Settings {
        protocols: crate::config::Protocols::both(),
        stack_dir: None,
        ..Settings::default()
    };
    let ctx = a_context()
        .engine(Arc::new(Reporting::default()))
        .over(Source::Embedded(&EMBEDDED))
        .settings(settings)
        .build();
    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };
    assert_eq!(
        dispatch(command, &ctx).await.err().map(|p| p.code),
        Some(crate::error::codes::stack::STACK_NOT_SET_UP),
        "an operator who has not run setup is told to, not shown a path error"
    );
}

#[tokio::test]
async fn a_stack_that_contradicts_itself_is_refused_with_every_fault_at_once() {
    let invalid = Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/invalid"
    )));
    let ctx = a_context()
        .engine(Arc::new(Reporting::default()))
        .over(invalid)
        .build();
    let command = Command::Up {
        forms: vec!["library".to_owned()],
    };

    let problem = dispatch(command, &ctx).await.err();
    assert_eq!(
        problem.as_ref().map(|problem| problem.code),
        Some(crate::error::codes::stack::STACK_INVALID)
    );

    let detail = problem
        .and_then(|problem| problem.detail)
        .unwrap_or_default();
    for expected in [
        "names profile telly, which is not declared",
        "that is not a pin",
        "not a recognised OSI identifier",
    ] {
        assert!(
            detail.contains(expected),
            "missing {expected:?} in: {detail}"
        );
    }
}
