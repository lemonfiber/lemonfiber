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
use lemonfiber_core::ports::docker::{Engine as _, Failure, Health, Lifecycle};

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
        let mut table: Vec<(String, Reply)> = vec![("/version".to_owned(), Reply::Body(200, version))];
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
        vec![("containers/json", fake::Reply::Body(200, LISTING.to_owned()))],
    );

    let listed = Daemon::at(&engine.socket).list("lemonfiber").await;
    assert_eq!(
        listed
            .ok()
            .map(|containers| containers
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
            ("containers/json", fake::Reply::Body(200, LISTING.to_owned())),
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
    if let Ok(mut lines) = daemon.logs("lemonfiber", query).await {
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
            ("containers/json", fake::Reply::Body(200, LISTING.to_owned())),
            ("/logs", fake::Reply::Multiplexed(Vec::new())),
        ],
    );

    let daemon = Daemon::at(&engine.socket);
    let query = lemonfiber_core::ports::docker::LogQuery {
        tail: 42,
        follow: true,
    };
    let mut seen = 0_usize;
    if let Ok(mut lines) = daemon.logs("lemonfiber", query).await {
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
            ("containers/json", fake::Reply::Body(200, LISTING.to_owned())),
            (
                "/logs",
                fake::Reply::Multiplexed(vec![(1, "still talking\n".to_owned())]),
            ),
        ],
    );

    let daemon = Daemon::at(&engine.socket);
    let query = lemonfiber_core::ports::docker::LogQuery::recent(10);
    let opened = daemon.logs("lemonfiber", query).await;
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
            ("containers/json", fake::Reply::Body(200, LISTING.to_owned())),
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
        !engine.asked_for().iter().any(|path| path.contains("id-gluetun/stats")),
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
            ("containers/json", fake::Reply::Body(200, LISTING.to_owned())),
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
            (
                "/exec",
                fake::Reply::Body(201, r#"{"Id":"e1"}"#.to_owned()),
            ),
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
            fake::Reply::Body(404, r#"{"message":"No such container: gluetun"}"#.to_owned()),
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
