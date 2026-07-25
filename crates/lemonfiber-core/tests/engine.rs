//! The engine adapter, driven against an engine written for the purpose.
//!
//! This is the one part of `lemonfiber-core` a trait fake cannot exercise. The
//! adapter's whole job is to speak the Engine API over a socket, so a fake
//! implementing `Engine` would prove only that the fake works. What gets
//! replaced here is the daemon: a socket answering with whatever a test wants
//! to say, which drives the connection, the request, the decoding and the
//! mapping in one pass — and needs no Docker installed to do it.
//!
//! It lives beside the crate rather than inside it because it is scaffolding
//! rather than product, and because scaffolding that must itself reach full
//! line coverage grows tests about the scaffolding.

use lemonfiber_core::adapters::Daemon;
use lemonfiber_core::ports::docker::{Engine as _, Failure, Health, Lifecycle, LogQuery};

/// An engine of our own, answering only what the adapter asks.
///
/// Deliberately not a general Docker implementation: it serves the handful of
/// routes this adapter uses and 404s the rest, because a fake that grew to
/// cover the whole API would need tests of its own.
#[cfg(unix)]
mod fake {
    use std::path::PathBuf;

    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::{UnixListener, UnixStream};
    use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
    use tokio::sync::oneshot::{channel, Receiver, Sender};
    use tokio::task::JoinHandle;

    /// What the engine says when a route is asked for.
    #[derive(Debug, Clone)]
    pub enum Reply {
        /// A complete body, under a status code.
        Body(u16, String),
        /// Docker's multiplexed stream framing, as logs arrive in.
        Multiplexed(Vec<(u8, String)>),
        /// The same framing, behind the protocol upgrade `exec` performs.
        Upgraded(Vec<(u8, String)>),
    }

    /// The API version this engine claims, which is deliberately not the one
    /// the adapter was compiled against — so a test can prove the two were
    /// reconciled rather than assumed.
    pub const CLAIMED_VERSION: &str = "1.44";

    /// One multiplexed frame: stream number, length, payload.
    fn frame(stream: u8, text: &str) -> Vec<u8> {
        let length = u32::try_from(text.len()).unwrap_or_default();
        let mut out = vec![stream, 0, 0, 0];
        out.extend_from_slice(&length.to_be_bytes());
        out.extend_from_slice(text.as_bytes());
        out
    }

    /// Everything a reply puts on the wire, headers included.
    fn rendered(reply: &Reply) -> Vec<u8> {
        match reply {
            Reply::Body(status, body) => {
                let head = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let mut out = head.into_bytes();
                out.extend_from_slice(body.as_bytes());
                out
            }
            Reply::Multiplexed(frames) => {
                let body: Vec<u8> = frames
                    .iter()
                    .flat_map(|(stream, text)| frame(*stream, text))
                    .collect();
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/vnd.docker.multiplexed-stream\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let mut out = head.into_bytes();
                out.extend_from_slice(&body);
                out
            }
            // An upgraded response has no length: the body is whatever arrives
            // until the connection closes, which is the point of upgrading.
            Reply::Upgraded(frames) => {
                let head = "HTTP/1.1 101 UPGRADED\r\nContent-Type: \
                            application/vnd.docker.multiplexed-stream\r\n\
                            Connection: Upgrade\r\nUpgrade: tcp\r\n\r\n";
                let mut out = head.as_bytes().to_vec();
                for (stream, text) in frames {
                    out.extend_from_slice(&frame(*stream, text));
                }
                out
            }
        }
    }

    /// Read one request, far enough to know what was asked for.
    ///
    /// Bodies are read and discarded rather than ignored: a server that answers
    /// before the client has finished sending leaves the client writing into a
    /// closed socket, which surfaces as a transport error in a test that was
    /// about something else entirely.
    async fn request(socket: &mut UnixStream) -> Option<String> {
        let mut received = Vec::new();
        let mut byte = [0_u8; 1];
        while !received.ends_with(b"\r\n\r\n") && socket.read(&mut byte).await.ok()? != 0 {
            received.push(byte[0]);
        }

        // Matched without regard to case, because header names are
        // case-insensitive and the client that will actually call this sends
        // them in lower case. Matching the spelling in the specification
        // instead is a body silently never read.
        let head = String::from_utf8_lossy(&received).into_owned();
        let length: usize = head
            .lines()
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            .and_then(|(_, value)| value.trim().parse().ok())
            .unwrap_or_default();
        if length > 0 {
            let mut body = vec![0_u8; length];
            socket.read_exact(&mut body).await.ok()?;
        }

        head.lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .map(ToOwned::to_owned)
    }

    /// An engine listening on a socket of its own.
    pub struct Engine {
        /// Where it is listening.
        pub socket: PathBuf,
        asked: UnboundedReceiver<String>,
        stopping: Option<Sender<()>>,
        serving: Option<JoinHandle<()>>,
    }

    impl Engine {
        /// Every route this engine was asked for, in order.
        pub fn asked_for(&mut self) -> Vec<String> {
            let mut seen = Vec::new();
            while let Ok(path) = self.asked.try_recv() {
                seen.push(path);
            }
            seen
        }

        /// Stop answering, and wait until it has actually stopped.
        ///
        /// Tests end by calling this rather than by walking away, so a socket
        /// is never still being served while the next test binds its own.
        pub async fn stop(mut self) {
            if let Some(stopping) = self.stopping.take() {
                let _ = stopping.send(());
            }
            if let Some(serving) = self.serving.take() {
                let _ = serving.await;
            }
        }
    }

    impl Drop for Engine {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.socket);
        }
    }

    /// Start an engine answering `routes`, matched as substrings of the path.
    ///
    /// The version route is always present, because the adapter settles the API
    /// version before it asks anything else and every test would otherwise have
    /// to declare that route itself.
    pub fn engine(name: &str, routes: Vec<(&'static str, Reply)>) -> Engine {
        // Not the platform's temporary directory: a Unix socket path is capped
        // near a hundred characters, and macOS puts per-user temporaries deep
        // enough to exceed it.
        let socket = PathBuf::from(format!("/tmp/lf-{}-{name}.sock", std::process::id()));
        let _ = std::fs::remove_file(&socket);

        let version = format!(r#"{{"ApiVersion":"{CLAIMED_VERSION}","Version":"29.4.0"}}"#);
        let mut table: Vec<(String, Reply)> =
            vec![("/version".to_owned(), Reply::Body(200, version))];
        table.extend(
            routes
                .into_iter()
                .map(|(path, reply)| (path.to_owned(), reply)),
        );

        let (asked, receiver) = unbounded_channel();
        let (stopping, stopped) = channel();
        let serving = UnixListener::bind(&socket)
            .ok()
            .map(|listener| tokio::spawn(answer(listener, table, asked, stopped)));

        Engine {
            socket,
            asked: receiver,
            stopping: Some(stopping),
            serving,
        }
    }

    /// Answer requests until asked to stop.
    async fn answer(
        listener: UnixListener,
        table: Vec<(String, Reply)>,
        asked: UnboundedSender<String>,
        mut stopped: Receiver<()>,
    ) {
        loop {
            tokio::select! {
                Ok((mut socket, _)) = listener.accept() => {
                    let table = table.clone();
                    let asked = asked.clone();
                    tokio::spawn(async move {
                        if let Some(path) = request(&mut socket).await {
                            let _ = asked.send(path.clone());

                            let found = table
                                .iter()
                                .find(|(route, _)| path.contains(route.as_str()))
                                .map(|(_, reply)| reply.clone());
                            let reply = found.unwrap_or(Reply::Body(
                                404,
                                r#"{"message":"no such route in this engine"}"#.to_owned(),
                            ));

                            let _ = socket.write_all(&rendered(&reply)).await;
                            let _ = socket.flush().await;
                            let _ = socket.shutdown().await;
                        }
                    });
                }
                _ = &mut stopped => break,
            }
        }
    }
}

/// Two containers as a listing, one running and one that fell over.
#[cfg(unix)]
const LISTING: &str = concat!(
    r#"[{"Id":"id-sonarr","#,
    r#""Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"sonarr"},"#,
    r#""State":"running","Status":"Up 2 minutes (healthy)","#,
    r#""Health":{"Status":"healthy"}},"#,
    r#"{"Id":"id-gluetun","#,
    r#""Labels":{"com.docker.compose.project":"lemonfiber","#,
    r#""com.docker.compose.service":"gluetun"},"#,
    r#""State":"exited","Status":"Exited (137) 2 hours ago"}]"#
);

#[cfg(unix)]
#[tokio::test]
async fn lists_what_the_engine_says_is_there_and_what_it_left_behind() {
    let engine = fake::engine(
        "list",
        vec![(
            "containers/json",
            fake::Reply::Body(200, LISTING.to_owned()),
        )],
    );

    let listed = Daemon::at(&engine.socket).list("lemonfiber").await;
    assert_eq!(
        listed.ok().map(|containers| containers
            .into_iter()
            .map(|found| (found.service, found.lifecycle, found.health, found.exit))
            .collect::<Vec<_>>()),
        Some(vec![
            (
                "sonarr".to_owned(),
                Lifecycle::Running,
                Health::Healthy,
                None
            ),
            (
                "gluetun".to_owned(),
                Lifecycle::Exited,
                Health::None,
                Some(137)
            ),
        ])
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn the_engine_s_own_api_version_is_agreed_before_anything_is_asked_of_it() {
    let mut engine = fake::engine(
        "negotiate",
        vec![("containers/json", fake::Reply::Body(200, "[]".to_owned()))],
    );

    let daemon = Daemon::at(&engine.socket);
    let listed = daemon.list("lemonfiber").await;
    assert_eq!(listed.ok().map(|containers| containers.len()), Some(0));

    // Asked twice, to prove the agreement is reached once and then kept. A
    // dashboard polling every second must not reopen that conversation
    // sixty times a minute.
    let _ = daemon.list("lemonfiber").await;

    let asked = engine.asked_for();
    assert_eq!(
        (
            asked.first().map(|path| path.contains("version")),
            asked.iter().filter(|path| path.contains("version")).count(),
            asked
                .iter()
                .filter(|path| path.contains("containers/json"))
                .count(),
        ),
        (Some(true), 1, 2),
        "the version is settled first, and settled once: {asked:?}"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn the_engine_is_asked_only_about_this_project_s_containers() {
    let mut engine = fake::engine(
        "filter",
        vec![("containers/json", fake::Reply::Body(200, "[]".to_owned()))],
    );

    let _ = Daemon::at(&engine.socket).list("housemedia").await;
    let listing = engine
        .asked_for()
        .into_iter()
        .find(|path| path.contains("containers/json"));

    assert_eq!(
        listing
            .as_deref()
            .map(|path| path.contains("compose.project") && path.contains("housemedia")),
        Some(true),
        "narrowing happens at the engine, not after nineteen containers crossed the socket: {listing:?}"
    );
    engine.stop().await;
}

#[tokio::test]
async fn an_engine_that_is_not_listening_is_reported_as_unreachable() {
    let nowhere = std::path::PathBuf::from("/tmp/lemonfiber-no-such-engine.sock");
    let outcome = Daemon::at(&nowhere).list("lemonfiber").await;
    assert!(
        matches!(outcome, Err(Failure::Unreachable { .. })),
        "{outcome:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_daemon_that_refuses_is_quoted_rather_than_paraphrased() {
    let engine = fake::engine(
        "refused",
        vec![(
            "containers/json",
            fake::Reply::Body(
                500,
                r#"{"message":"permission denied while trying to connect"}"#.to_owned(),
            ),
        )],
    );

    let outcome = Daemon::at(&engine.socket).list("lemonfiber").await;
    assert_eq!(
        outcome.err().map(|failure| failure.to_string()),
        Some(
            "the container engine is not reachable: \
             permission denied while trying to connect"
                .to_owned()
        ),
        "the daemon's own sentence is what the operator needs to read"
    );
    engine.stop().await;
}

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
    let query = lemonfiber_core::ports::docker::LogQuery::recent(20);

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
    let query = lemonfiber_core::ports::docker::LogQuery {
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
    let query = lemonfiber_core::ports::docker::LogQuery::recent(10);
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
    // Sampling starts by finding the containers, so an absent engine refuses
    // here as surely as it refuses a listing — a dashboard must be told the
    // telemetry is gone rather than shown a panel that never fills in.
    let nowhere = std::path::PathBuf::from("/tmp/lemonfiber-no-such-engine.sock");
    let outcome = Daemon::at(&nowhere).stats("lemonfiber").await;
    assert!(
        matches!(outcome, Err(Failure::Unreachable { .. })),
        "{:?}",
        outcome.err()
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

#[cfg(unix)]
#[tokio::test]
async fn a_command_run_inside_a_container_reports_what_it_wrote() {
    let engine = fake::engine(
        "exec",
        vec![
            (
                "/exec/e1/start",
                fake::Reply::Upgraded(vec![(1, "203.0.113.7\n".to_owned())]),
            ),
            (
                "/exec/e1/json",
                fake::Reply::Body(200, r#"{"ExitCode":0}"#.to_owned()),
            ),
            ("/exec", fake::Reply::Body(201, r#"{"Id":"e1"}"#.to_owned())),
        ],
    );

    let argv = ["curl", "-s", "https://ifconfig.me"].map(str::to_owned);
    let ran = Daemon::at(&engine.socket).exec("gluetun", &argv).await;

    assert_eq!(
        ran.ok().map(|output| (output.status, output.stdout)),
        Some((Some(0), "203.0.113.7\n".to_owned())),
        "this is the shape the leak test compares two of"
    );
    engine.stop().await;
}

#[cfg(unix)]
#[tokio::test]
async fn a_command_aimed_at_a_container_that_is_not_there_says_which() {
    let engine = fake::engine(
        "no-container",
        vec![(
            "/exec",
            fake::Reply::Body(
                404,
                r#"{"message":"No such container: gluetun"}"#.to_owned(),
            ),
        )],
    );

    let argv = ["true".to_owned()];
    let outcome = Daemon::at(&engine.socket).exec("gluetun", &argv).await;
    assert!(
        matches!(&outcome, Err(Failure::NoSuchContainer { name }) if name == "gluetun"),
        "{outcome:?}"
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

#[tokio::test]
async fn the_engine_this_machine_is_configured_for_is_reachable_or_reported_absent() {
    // Whether a daemon is running here is not this test's business. Either
    // answer is correct; what must never happen is the local address going
    // unexercised, or an absent daemon being reported as something an operator
    // would go looking for a container about.
    let daemon = Daemon::local();
    assert!(
        format!("{daemon:?}").contains("Local"),
        "an adapter has to say which engine it is pointed at, for a bundle to be worth reading"
    );

    let outcome = daemon.list("lemonfiber-no-such-project").await;
    assert!(
        matches!(outcome, Ok(_) | Err(Failure::Unreachable { .. })),
        "{outcome:?}"
    );
}
