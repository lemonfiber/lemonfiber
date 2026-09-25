//! Logs and resource use streamed from the engine, and stopped when nobody reads.

use super::{fake, LISTING};
use lemonfiber_adapters::Daemon;
use lemonfiber_ports::docker::{Engine as _, Failure, Health, Lifecycle, LogQuery};

#[cfg(unix)]
#[tokio::test]
async fn log_lines_are_tagged_with_the_service_and_the_stream_they_came_from() {
    let engine = fake::engine(
        "logs",
        vec![
            (
                "containers/json",
                fake::Reply::Body(200, LISTING.to_owned()),
            ),
            (
                "/logs",
                fake::Reply::Multiplexed(vec![
                    (1, "2026-07-25T18:40:55Z import complete\n".to_owned()),
                    (2, "2026-07-25T18:40:56Z database is locked\n".to_owned()),
                ]),
            ),
        ],
    );

    let daemon = Daemon::at(&engine.socket);
    let query = lemonfiber_ports::docker::LogQuery::recent(20);

    // Sorted for comparison only. Two services producing at once arrive
    // interleaved, which is the feature, and is why each line carries the
    // service and the instant its own container put on it.
    let mut seen = Vec::new();
    if let Ok(mut lines) = daemon.logs("lemonfiber", &[], query).await {
        while let Some(line) = lines.recv().await {
            seen.push((
                line.service,
                format!("{:?}", line.stream),
                line.at,
                line.line,
            ));
        }
    }
    seen.sort();

    assert_eq!(
        seen,
        vec![
            (
                "gluetun".to_owned(),
                "Stderr".to_owned(),
                Some("2026-07-25T18:40:56Z".to_owned()),
                "database is locked".to_owned()
            ),
            (
                "gluetun".to_owned(),
                "Stdout".to_owned(),
                Some("2026-07-25T18:40:55Z".to_owned()),
                "import complete".to_owned()
            ),
            (
                "sonarr".to_owned(),
                "Stderr".to_owned(),
                Some("2026-07-25T18:40:56Z".to_owned()),
                "database is locked".to_owned()
            ),
            (
                "sonarr".to_owned(),
                "Stdout".to_owned(),
                Some("2026-07-25T18:40:55Z".to_owned()),
                "import complete".to_owned()
            ),
        ],
        "a stopped service still has scrollback worth reading"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn how_much_output_to_ask_for_reaches_the_engine() {
    let mut engine = fake::engine(
        "tail",
        vec![
            (
                "containers/json",
                fake::Reply::Body(200, LISTING.to_owned()),
            ),
            ("/logs", fake::Reply::Multiplexed(Vec::new())),
        ],
    );

    let daemon = Daemon::at(&engine.socket);
    let query = lemonfiber_ports::docker::LogQuery {
        tail: 42,
        follow: true,
    };
    let mut seen = 0_usize;
    if let Ok(mut lines) = daemon.logs("lemonfiber", &[], query).await {
        while lines.recv().await.is_some() {
            seen += 1;
        }
    }
    assert_eq!(seen, 0, "this engine was given nothing to say");

    let asked = engine
        .asked_for()
        .into_iter()
        .find(|path| path.contains("/logs"));
    assert_eq!(
        asked.as_deref().map(|path| (
            path.contains("tail=42"),
            path.contains("follow=true"),
            path.contains("timestamps=true")
        )),
        Some((true, true, true)),
        "{asked:?}"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_reader_that_walks_away_stops_the_producer_rather_than_the_process() {
    let engine = fake::engine(
        "abandoned",
        vec![
            (
                "containers/json",
                fake::Reply::Body(200, LISTING.to_owned()),
            ),
            (
                "/logs",
                fake::Reply::Multiplexed(vec![(1, "still talking\n".to_owned())]),
            ),
        ],
    );

    let daemon = Daemon::at(&engine.socket);
    let query = lemonfiber_ports::docker::LogQuery::recent(10);
    let opened = daemon.logs("lemonfiber", &[], query).await;
    assert!(opened.is_ok());

    // Closing the panel is ordinary. The producers must notice and stop,
    // which they can only do by trying to send into a closed channel.
    drop(opened);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn resource_use_is_sampled_for_the_services_that_are_running() {
    let sample = concat!(
        r#"{"name":"/sonarr","cpu_stats":{"cpu_usage":{"total_usage":500},"#,
        r#""system_cpu_usage":1000,"online_cpus":1},"#,
        r#""precpu_stats":{"cpu_usage":{"total_usage":0},"system_cpu_usage":0},"#,
        r#""memory_stats":{"usage":4096}}"#,
        "\n"
    );

    let mut engine = fake::engine(
        "stats",
        vec![
            (
                "containers/json",
                fake::Reply::Body(200, LISTING.to_owned()),
            ),
            ("/stats", fake::Reply::Body(200, sample.to_owned())),
        ],
    );

    let daemon = Daemon::at(&engine.socket);
    let mut seen = Vec::new();
    if let Ok(mut samples) = daemon.stats("lemonfiber").await {
        while let Some((service, stats)) = samples.recv().await {
            seen.push((service, format!("{:.2}", stats.cpu), stats.memory_bytes));
        }
    }

    assert_eq!(
        seen,
        vec![("sonarr".to_owned(), "0.50".to_owned(), 4096)],
        "the stopped service is not sampled, because it is not using anything"
    );
    assert!(
        !engine
            .asked_for()
            .iter()
            .any(|path| path.contains("id-gluetun/stats")),
        "a stopped container is never asked how busy it is"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_sampler_nobody_is_reading_stops_as_well() {
    let engine = fake::engine(
        "unsampled",
        vec![
            (
                "containers/json",
                fake::Reply::Body(200, LISTING.to_owned()),
            ),
            (
                "/stats",
                fake::Reply::Body(200, "{\"memory_stats\":{\"usage\":1}}\n".to_owned()),
            ),
        ],
    );

    let opened = Daemon::at(&engine.socket).stats("lemonfiber").await;
    assert!(opened.is_ok());
    drop(opened);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    engine.stop().await;
}

#[tokio::test]
async fn resource_use_cannot_be_sampled_from_an_engine_that_is_not_there() {
    // Sampling reaches for the connection before it asks anything over it, so an
    // absent engine refuses here as surely as it refuses a listing — a dashboard
    // must be told the telemetry is gone rather than shown a panel that never
    // fills in.
    let nowhere = std::path::PathBuf::from("/tmp/lemonfiber-no-such-engine.sock");
    let outcome = Daemon::at(&nowhere).stats("lemonfiber").await;
    assert!(
        matches!(outcome, Err(Failure::Unreachable { .. })),
        "{:?}",
        outcome.err()
    );
}

/// A log viewer is told the engine is down rather than shown an empty scrollback.
///
/// The same reach as the sampling above it, and the same reason: an operator who
/// opened the logs because something is wrong would read emptiness as the service
/// having written nothing, which is the opposite of what happened.
#[tokio::test]
async fn output_cannot_be_followed_from_an_engine_that_is_not_there() {
    let nowhere = std::path::PathBuf::from("/tmp/lemonfiber-no-such-engine.sock");

    let outcome = Daemon::at(&nowhere)
        .logs("lemonfiber", &[], LogQuery::recent(10))
        .await;

    assert!(
        matches!(outcome, Err(Failure::Unreachable { .. })),
        "an engine that is not there has no scrollback to be empty"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn naming_services_opens_no_stream_for_the_ones_nobody_asked_about() {
    let mut engine = fake::engine(
        "narrowed",
        vec![
            (
                "containers/json",
                fake::Reply::Body(200, LISTING.to_owned()),
            ),
            (
                "/logs",
                fake::Reply::Multiplexed(vec![(1, "2026-07-25T18:40:55Z hello\n".to_owned())]),
            ),
        ],
    );

    let daemon = Daemon::at(&engine.socket);
    let wanted = ["sonarr".to_owned()];
    let mut seen = Vec::new();
    if let Ok(mut lines) = daemon
        .logs("lemonfiber", &wanted, LogQuery::recent(10))
        .await
    {
        while let Some(line) = lines.recv().await {
            seen.push(line.service);
        }
    }

    assert_eq!(seen, vec!["sonarr".to_owned()]);
    assert!(
        !engine
            .asked_for()
            .iter()
            .any(|path| path.contains("id-gluetun/logs")),
        "narrowing means the stream is never opened, not that its lines are dropped"
    );
    engine.stop().await;
}

/// An engine that will not say what is running opens no streams either.
///
/// Both stream readers start by asking which containers there are, and a dashboard
/// told nothing is running would show nineteen empty panels rather than a fault.
/// Driven through one daemon, because the connection is settled once and what fails
/// here is the question asked over it.
#[cfg(unix)]
#[tokio::test]
async fn an_engine_that_will_not_say_what_is_running_opens_no_streams() {
    let engine = fake::engine(
        "streams-refused",
        vec![(
            "containers/json",
            fake::Reply::Body(500, r#"{"message":"database is locked"}"#.to_owned()),
        )],
    );
    let daemon = Daemon::at(&engine.socket);

    let sampled = daemon.stats("lemonfiber").await;
    let followed = daemon.logs("lemonfiber", &[], LogQuery::recent(10)).await;

    assert!(
        matches!(sampled, Err(Failure::Unreachable { .. })),
        "telemetry gone is a thing to say, not a panel that never fills in"
    );
    assert!(
        matches!(followed, Err(Failure::Unreachable { .. })),
        "and so is a log viewer with nothing in it"
    );
    engine.stop().await;
}

/// Every state and verdict the engine can report, in one listing.
///
/// Driven through `list` rather than against the mapping functions directly.
/// The mapping is not the behaviour; what an operator gets when the engine says
/// `stopping` is, and a test that reaches past the port cannot tell the
/// difference between the two being wrong.
#[cfg(unix)]
const STATES: &str = concat!(
    r#"[{"Id":"id-created","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"created"},"State":"created","Status":"Created"},"#,
    r#"{"Id":"id-paused","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"paused"},"State":"paused","Status":"Up (Paused)"},"#,
    r#"{"Id":"id-looping","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"looping"},"State":"restarting","#,
    r#""Status":"Restarting (1) 2 seconds ago"},"#,
    r#"{"Id":"id-removing","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"removing"},"State":"removing","#,
    r#""Status":"Removal In Progress"},"#,
    r#"{"Id":"id-stopping","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"stopping"},"State":"stopping","Status":"Stopping"},"#,
    r#"{"Id":"id-dead","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"dead"},"State":"dead","Status":"Dead"},"#,
    r#"{"Id":"id-blank","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"blank"},"State":"","Status":""},"#,
    r#"{"Id":"id-silent","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"silent"}},"#,
    r#"{"Id":"id-warming","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"warming"},"State":"running","#,
    r#""Status":"Up 1 second (health: starting)","Health":{"Status":"starting"}},"#,
    r#"{"Id":"id-failing","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"failing"},"State":"running","#,
    r#""Status":"Up 1 minute (unhealthy)","Health":{"Status":"unhealthy"}},"#,
    r#"{"Id":"id-unprobed","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"unprobed"},"State":"running","#,
    r#""Status":"Up 3 hours","Health":{"Status":"none"}},"#,
    r#"{"Id":"id-killed","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"killed"},"State":"exited","#,
    r#""Status":"Exited (137) 2 hours ago","Health":{"Status":""}},"#,
    r#"{"Id":"id-odd","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"odd"},"State":"exited","Status":"Exited )backwards("}]"#
);

#[cfg(unix)]
#[tokio::test]
async fn every_state_and_verdict_the_engine_reports_arrives_intact() {
    let engine = fake::engine(
        "states",
        vec![("containers/json", fake::Reply::Body(200, STATES.to_owned()))],
    );

    let listed = Daemon::at(&engine.socket)
        .list("lemonfiber")
        .await
        .unwrap_or_default();
    let seen: Vec<(String, Lifecycle, Health, Option<i32>)> = listed
        .into_iter()
        .map(|found| (found.service, found.lifecycle, found.health, found.exit))
        .collect();

    assert_eq!(
        seen,
        vec![
            ("created".to_owned(), Lifecycle::Created, Health::None, None),
            ("paused".to_owned(), Lifecycle::Paused, Health::None, None),
            (
                "looping".to_owned(),
                Lifecycle::Restarting,
                Health::None,
                Some(1)
            ),
            (
                "removing".to_owned(),
                Lifecycle::Removing,
                Health::None,
                None
            ),
            (
                "stopping".to_owned(),
                Lifecycle::Removing,
                Health::None,
                None
            ),
            ("dead".to_owned(), Lifecycle::Dead, Health::None, None),
            // A state the engine will not name, and a state it did not send at
            // all. Silence reads as dead rather than as running: the honest
            // reading is that nothing is known to be up.
            ("blank".to_owned(), Lifecycle::Dead, Health::None, None),
            ("silent".to_owned(), Lifecycle::Dead, Health::None, None),
            (
                "warming".to_owned(),
                Lifecycle::Running,
                Health::Starting,
                None
            ),
            (
                "failing".to_owned(),
                Lifecycle::Running,
                Health::Unhealthy,
                None
            ),
            (
                "unprobed".to_owned(),
                Lifecycle::Running,
                Health::None,
                None
            ),
            (
                "killed".to_owned(),
                Lifecycle::Exited,
                Health::None,
                Some(137)
            ),
            // Parentheses in the wrong order are not an exit code, and guessing
            // one would invent a fault that never happened.
            ("odd".to_owned(), Lifecycle::Exited, Health::None, None),
        ]
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_line_the_container_did_not_timestamp_is_passed_through_whole() {
    let engine = fake::engine(
        "stamps",
        vec![
            (
                "containers/json",
                fake::Reply::Body(200, LISTING.to_owned()),
            ),
            (
                "id-sonarr/logs",
                fake::Reply::Multiplexed(vec![(
                    1,
                    concat!(
                        "2026-07-25T18:40:55.123456789Z import complete\n",
                        "no timestamp here\n",
                        "single\n",
                        "2026-07-25 not-a-stamp\n"
                    )
                    .to_owned(),
                )]),
            ),
            ("id-gluetun/logs", fake::Reply::Multiplexed(Vec::new())),
        ],
    );

    let daemon = Daemon::at(&engine.socket);
    let mut seen = Vec::new();
    if let Ok(mut lines) = daemon.logs("lemonfiber", &[], LogQuery::recent(10)).await {
        while let Some(line) = lines.recv().await {
            seen.push((line.at, line.line));
        }
    }

    assert_eq!(
        seen,
        vec![
            (
                Some("2026-07-25T18:40:55.123456789Z".to_owned()),
                "import complete".to_owned()
            ),
            (None, "no timestamp here".to_owned()),
            (None, "single".to_owned()),
            // A date is not an instant. Half a message rendered as a timestamp
            // is worse than no timestamp at all.
            (None, "2026-07-25 not-a-stamp".to_owned()),
        ]
    );
    engine.stop().await;
}

/// A listing of three running containers, for sampling each differently.
#[cfg(unix)]
const SAMPLED: &str = concat!(
    r#"[{"Id":"id-busy","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"busy"},"State":"running"},"#,
    r#"{"Id":"id-first","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"first"},"State":"running"},"#,
    r#"{"Id":"id-quiet","Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"quiet"},"State":"running"}]"#
);

#[cfg(unix)]
#[tokio::test]
async fn a_rate_is_only_reported_where_there_are_two_samples_to_compute_it_from() {
    let busy = concat!(
        r#"{"cpu_stats":{"cpu_usage":{"total_usage":500},"system_cpu_usage":1000,"#,
        r#""online_cpus":1},"precpu_stats":{"cpu_usage":{"total_usage":0},"#,
        r#""system_cpu_usage":0},"memory_stats":{"usage":4096}}"#,
        "\n"
    );
    // The very first sample of a container: no time has passed between the two
    // readings, so any rate would be invented rather than measured.
    let first = concat!(
        r#"{"cpu_stats":{"cpu_usage":{"total_usage":500},"system_cpu_usage":1000},"#,
        r#""precpu_stats":{"cpu_usage":{"total_usage":0},"system_cpu_usage":1000},"#,
        r#""memory_stats":{"usage":8}}"#,
        "\n"
    );
    let quiet = "{}\n";

    let engine = fake::engine(
        "rates",
        vec![
            (
                "containers/json",
                fake::Reply::Body(200, SAMPLED.to_owned()),
            ),
            ("id-busy/stats", fake::Reply::Body(200, busy.to_owned())),
            ("id-first/stats", fake::Reply::Body(200, first.to_owned())),
            ("id-quiet/stats", fake::Reply::Body(200, quiet.to_owned())),
        ],
    );

    let daemon = Daemon::at(&engine.socket);
    let mut seen = Vec::new();
    if let Ok(mut samples) = daemon.stats("lemonfiber").await {
        while let Some((service, stats)) = samples.recv().await {
            seen.push((service, format!("{:.2}", stats.cpu), stats.memory_bytes));
        }
    }
    seen.sort();

    assert_eq!(
        seen,
        vec![
            ("busy".to_owned(), "0.50".to_owned(), 4096),
            ("first".to_owned(), "0.00".to_owned(), 8),
            // An engine that said nothing is not a container using nothing, but
            // zero is the only number that is not a guess.
            ("quiet".to_owned(), "0.00".to_owned(), 0),
        ]
    );
    engine.stop().await;
}
