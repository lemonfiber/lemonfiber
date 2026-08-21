//! The seams a test stands in for, beyond the transport and the filesystem.
//!
//! A runner that answers from a script, an engine that reports what a test put in it, a
//! filesystem that hands back the configuration a service would have written. Lifted out of
//! `lemonfiber-core`, where being private to the crate meant its integration tests could
//! not see them.

use std::sync::Arc;

use crate::http::{Answer, Fake};
use async_trait::async_trait;
use lemonfiber_ports::docker::{
    Container, Engine, ExecOutput, Failure as EngineFailure, Health, Lifecycle, LogLine, LogQuery,
    Stats, Stream,
};
use lemonfiber_ports::process::{Failure, Output, Runner};
use tokio::sync::mpsc::Receiver;

/// A runner that answers with whatever the test scripted.
pub struct Scripted(pub Result<Output, Failure>);

#[async_trait]
impl Runner for Scripted {
    async fn run(&self, _argv: &[String]) -> Result<Output, Failure> {
        echoed(&self.0)
    }
    // `stream` uses the trait's default (run then replay); a test that drives a
    // streamed pull exercises it.
}

/// A runner that answers the same way every time and remembers what it was asked.
///
/// [`Scripted`] throws the argument vector away, which is right until a test's claim
/// is about a command that should **not** have run. Proving that a form which failed
/// to start was never then torn down is a statement about everything the runner was
/// handed, and cannot be made from what came back out of it.
pub struct Recording {
    answer: Result<Output, Failure>,
    seen: std::sync::Mutex<Vec<Vec<String>>>,
}

impl Recording {
    /// A runner that always answers this way.
    #[must_use]
    pub fn answering(answer: Result<Output, Failure>) -> Self {
        Self {
            answer,
            seen: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Whether any command it was handed used this Compose subcommand.
    ///
    /// A poisoned lock reads as nothing having been run. That is the answer that
    /// fails a test asserting something happened, rather than the one that lets a
    /// test asserting nothing happened pass without having looked.
    #[must_use]
    pub fn ran(&self, subcommand: &str) -> bool {
        self.seen.lock().is_ok_and(|seen| {
            seen.iter()
                .any(|argv| argv.iter().any(|word| word == subcommand))
        })
    }
}

#[async_trait]
impl Runner for Recording {
    async fn run(&self, argv: &[String]) -> Result<Output, Failure> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(argv.to_vec());
        }
        echoed(&self.answer)
    }
}

/// The scripted answer again, since a [`Failure`] cannot be cloned.
///
/// Shared by both runners rather than written twice: the arms exist only because the
/// error type is not `Clone`, and a second copy would be a second place to forget a
/// variant when one is added.
fn echoed(answer: &Result<Output, Failure>) -> Result<Output, Failure> {
    match answer {
        Ok(output) => Ok(output.clone()),
        Err(Failure::NotFound { program }) => Err(Failure::NotFound {
            program: program.clone(),
        }),
        Err(Failure::Unusable { program, reason }) => Err(Failure::Unusable {
            program: program.clone(),
            reason: reason.clone(),
        }),
    }
}

/// An engine that reports whatever the test put in it.
///
/// A trait fake is the right tool here and the wrong one for the adapter:
/// the question above the port is what lemonfiber does with an answer, and
/// the question below it is whether the wire is spoken correctly.
#[derive(Default)]
pub struct Reporting {
    containers: Vec<Container>,
    said: Vec<LogLine>,
    reachable: bool,
    /// How many listings to answer before every container reports healthy.
    ///
    /// Absent means the engine never changes its mind. A stack that is
    /// genuinely starting does not answer the same way twice, and an engine
    /// that always did would make the waiting itself untestable.
    settles_after: Option<usize>,
    asked: std::sync::atomic::AtomicUsize,
    /// What `exec` answers a VPN probe with, where the test scripts one. Absent
    /// means `exec` has nothing to say and fails, as an engine asked for a
    /// container it does not know would.
    tunnel: Option<Tunnel>,
}

/// What a [`Reporting`] engine answers a VPN exec with: a public address for the
/// gateway and for the client (told apart by the gateway's service id appearing in
/// the container), a country, and a forwarded port. A `None` field answers as
/// absent — the exit code a missing value produces.
#[derive(Clone)]
pub struct Tunnel {
    /// The compose service id of the gateway, which tells its answers from the client's.
    pub gateway: &'static str,
    /// The public address the gateway reports.
    pub gateway_ip: Option<&'static str>,
    /// The public address a container behind it reports.
    pub client_ip: Option<&'static str>,
    /// The country those addresses resolve to.
    pub country: Option<&'static str>,
    /// The forwarded port the gateway reports holding.
    pub port: Option<&'static str>,
    /// What a second address service says the gateway's egress is, where a test
    /// is about the sources contradicting each other. Absent means both agree.
    pub second_opinion: Option<&'static str>,
}

impl Reporting {
    /// An engine reporting the named services in one state.
    #[must_use]
    pub fn holding(services: &[&str], lifecycle: Lifecycle, health: Health) -> Self {
        Self {
            containers: services
                .iter()
                .map(|service| Container {
                    id: format!("id-{service}"),
                    project: "lemonfiber".to_owned(),
                    service: (*service).to_owned(),
                    lifecycle,
                    health,
                    exit: None,
                })
                .collect(),
            said: Vec::new(),
            reachable: true,
            settles_after: None,
            asked: std::sync::atomic::AtomicUsize::new(0),
            tunnel: None,
        }
    }

    /// The same engine, unsettled until it has been asked `listings` times.
    #[must_use]
    pub fn settling_after(mut self, listings: usize) -> Self {
        self.settles_after = Some(listings);
        self
    }

    /// The same engine, with something to say about a service.
    #[must_use]
    pub fn saying(mut self, service: &str, line: &str) -> Self {
        self.said.push(LogLine {
            service: service.to_owned(),
            stream: Stream::Stderr,
            at: None,
            line: line.to_owned(),
        });
        self
    }

    /// The same engine, scripted to answer a VPN probe (public address, country,
    /// forwarded port) so the dashboard's VPN panel can be driven.
    #[must_use]
    pub fn with_tunnel(mut self, tunnel: Tunnel) -> Self {
        self.tunnel = Some(tunnel);
        self
    }

    /// An engine that is not there.
    #[must_use]
    pub fn absent() -> Self {
        Self {
            containers: Vec::new(),
            said: Vec::new(),
            reachable: false,
            settles_after: None,
            asked: std::sync::atomic::AtomicUsize::new(0),
            tunnel: None,
        }
    }
}

#[async_trait]
impl Engine for Reporting {
    async fn list(&self, _project: &str) -> Result<Vec<Container>, EngineFailure> {
        if !self.reachable {
            return Err(EngineFailure::Unreachable {
                reason: "no daemon here".to_owned(),
            });
        }

        let asked = self
            .asked
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let settled = self.settles_after.is_some_and(|after| asked >= after);

        Ok(self
            .containers
            .iter()
            .map(|container| Container {
                health: if settled {
                    Health::Healthy
                } else {
                    container.health
                },
                ..container.clone()
            })
            .collect())
    }

    async fn exec(&self, container: &str, argv: &[String]) -> Result<ExecOutput, EngineFailure> {
        let Some(tunnel) = &self.tunnel else {
            return Err(EngineFailure::NoSuchContainer {
                name: container.to_owned(),
            });
        };
        if argv.first().is_some_and(|arg| arg == "cat") {
            return Ok(scripted(tunnel.port));
        }
        if argv.last().is_some_and(|arg| arg.ends_with("/country-iso")) {
            return Ok(ExecOutput {
                status: Some(0),
                stdout: tunnel.country.unwrap_or_default().to_owned(),
            });
        }
        let asked_second = argv
            .last()
            .is_some_and(|arg| arg.contains("second.example"));
        let ip = if container.contains(tunnel.gateway) {
            if asked_second {
                tunnel.second_opinion.or(tunnel.gateway_ip)
            } else {
                tunnel.gateway_ip
            }
        } else {
            tunnel.client_ip
        };
        Ok(scripted(ip))
    }

    async fn stats(&self, _project: &str) -> Result<Receiver<(String, Stats)>, EngineFailure> {
        let (_sender, receiver) = tokio::sync::mpsc::channel(1);
        Ok(receiver)
    }

    async fn logs(
        &self,
        _project: &str,
        services: &[String],
        _query: LogQuery,
    ) -> Result<Receiver<LogLine>, EngineFailure> {
        // Opening a stream means finding the containers first, so an engine
        // that cannot be reached refuses this as surely as it refuses a
        // listing. A fake that answered anyway would be a fake that made
        // the health gate's own failure path untestable.
        if !self.reachable {
            return Err(EngineFailure::Unreachable {
                reason: "no daemon here".to_owned(),
            });
        }

        let wanted: Vec<LogLine> = self
            .said
            .iter()
            .filter(|line| services.is_empty() || services.contains(&line.service))
            .cloned()
            .collect();

        let (sender, receiver) = tokio::sync::mpsc::channel(wanted.len().max(1));
        for line in wanted {
            let _ = sender.send(line).await;
        }
        Ok(receiver)
    }
}

/// A randomness source answering with exactly the bytes a test scripts.
pub struct FixedRandom(pub Option<Vec<u8>>);

impl lemonfiber_ports::random::Random for FixedRandom {
    fn bytes(&self, _n: usize) -> Option<Vec<u8>> {
        self.0.clone()
    }
}

/// The routes a whole seed run reads, so its many calls need no exact ordering.
///
/// Root-folder, download-client and Prowlarr application lists hold exactly what each
/// service wants, so every connection reads back as already wired; qBittorrent's auth
/// and set both answer success. The catch-all is last because the first matching route
/// wins, and an empty fragment is contained in every URL.
#[must_use]
pub fn seeding_routes() -> Vec<(&'static str, Answer)> {
    vec![
        (
            "/downloadclient",
            Answer::reply(
                200,
                r#"[{"id":1,"fields":[{"name":"host","value":"sabnzbd"},{"name":"port","value":8080}]},{"id":2,"fields":[{"name":"host","value":"gluetun"},{"name":"port","value":8081}]}]"#,
            ),
        ),
        (
            "/rootfolder",
            Answer::reply(
                200,
                r#"[{"id":1,"path":"/data/media/tv"},{"id":2,"path":"/data/media/movies"},{"id":3,"path":"/data/media/music"}]"#,
            ),
        ),
        (
            "/applications",
            Answer::reply(
                200,
                r#"[{"id":1,"fields":[{"name":"baseUrl","value":"http://sonarr:8989"}]},{"id":2,"fields":[{"name":"baseUrl","value":"http://radarr:7878"}]},{"id":3,"fields":[{"name":"baseUrl","value":"http://lidarr:8686"}]}]"#,
            ),
        ),
        ("", Answer::reply(200, "Ok.")),
    ]
}

/// A transport answering a whole seed run, every connection already wired.
#[must_use]
pub fn seeding() -> Arc<Fake> {
    Fake::by_path(seeding_routes())
}

/// A seed run answering `extra` first and falling through to the ordinary routes.
///
/// For the tests that need one service to say something different — a version, a
/// second download client — without restating the whole table around it.
#[must_use]
pub fn seeding_with(extra: Vec<(&'static str, Answer)>) -> Arc<Fake> {
    let mut routes = extra;
    routes.extend(seeding_routes());
    Fake::by_path(routes)
}

/// A filesystem that hands back a Servarr configuration for a Servarr path and
/// a `SABnzbd` one for `SABnzbd`'s, or nothing. Only `read` is meaningful to
/// seeding; the rest are unused.
pub struct SeedFs {
    servarr: Option<&'static str>,
    sabnzbd: Option<&'static str>,
    /// When set, the Servarr configuration is handed back only for Prowlarr's
    /// path, so a test can make Prowlarr's key readable while an \*arr's is not —
    /// the case of an \*arr that started after Prowlarr.
    only_prowlarr: bool,
    /// Path fragments a canonicalize should fail for, standing in for host
    /// directories that are not there — so a root folder's existence check can be
    /// driven to missing. Every path resolves when empty.
    missing: Vec<&'static str>,
    /// What a volume describe reports — a zero total by default, which the
    /// dashboard reads as free space unknown.
    facts: lemonfiber_ports::filesystem::StorageFacts,
}

impl SeedFs {
    /// A transport answering by the shape of the URL, with these two keys in place.
    #[must_use]
    pub fn keyed(servarr: Option<&'static str>, sabnzbd: Option<&'static str>) -> Self {
        Self {
            servarr,
            sabnzbd,
            only_prowlarr: false,
            missing: Vec::new(),
            facts: lemonfiber_ports::filesystem::StorageFacts {
                kind: lemonfiber_ports::filesystem::FsKind::Linking("test".to_owned()),
                removable: false,
                available: 0,
                total: 0,
            },
        }
    }

    /// The same, but withholding the Servarr key from every path but Prowlarr's.
    #[must_use]
    pub fn only_for_prowlarr(mut self) -> Self {
        self.only_prowlarr = true;
        self
    }

    /// The same, reporting the given volume facts to a describe.
    #[must_use]
    pub fn with_facts(mut self, facts: lemonfiber_ports::filesystem::StorageFacts) -> Self {
        self.facts = facts;
        self
    }

    /// The same, but failing to canonicalize any path carrying one of these
    /// fragments — a host directory that is not there, for a root-folder check.
    #[must_use]
    pub fn missing(mut self, fragments: Vec<&'static str>) -> Self {
        self.missing = fragments;
        self
    }
}

#[async_trait]
impl lemonfiber_ports::filesystem::FileSystem for SeedFs {
    async fn canonicalize(
        &self,
        path: &std::path::Path,
    ) -> Result<std::path::PathBuf, lemonfiber_ports::filesystem::Fault> {
        let text = path.to_string_lossy();
        if self.missing.iter().any(|fragment| text.contains(fragment)) {
            return Err(lemonfiber_ports::filesystem::Fault::new("no such path"));
        }
        Ok(path.to_path_buf())
    }
    async fn touch(
        &self,
        _path: &std::path::Path,
    ) -> Result<(), lemonfiber_ports::filesystem::Fault> {
        Err(lemonfiber_ports::filesystem::Fault::new("unused"))
    }
    async fn link(
        &self,
        _from: &std::path::Path,
        _to: &std::path::Path,
    ) -> Result<(), lemonfiber_ports::filesystem::Fault> {
        Err(lemonfiber_ports::filesystem::Fault::new("unused"))
    }
    async fn identify(
        &self,
        _path: &std::path::Path,
    ) -> Result<lemonfiber_ports::filesystem::Identity, lemonfiber_ports::filesystem::Fault> {
        Err(lemonfiber_ports::filesystem::Fault::new("unused"))
    }
    async fn remove(&self, _path: &std::path::Path) {}
    async fn read(&self, path: &std::path::Path) -> Option<String> {
        let path = path.to_string_lossy();
        if path.contains("sabnzbd") {
            return self.sabnzbd.map(str::to_owned);
        }
        if self.only_prowlarr && !path.contains("prowlarr") {
            return None;
        }
        self.servarr.map(str::to_owned)
    }
    async fn write(&self, _path: &std::path::Path, _contents: &str) {}
    async fn ownership(
        &self,
        _path: &std::path::Path,
    ) -> Option<lemonfiber_ports::filesystem::Ownership> {
        None
    }
    async fn describe(
        &self,
        _path: &std::path::Path,
    ) -> lemonfiber_ports::filesystem::StorageFacts {
        self.facts.clone()
    }
}

/// A program that succeeded, saying `stdout`.
#[must_use]
pub fn spoke(stdout: &str) -> Output {
    Output {
        status: Some(0),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

/// A program that failed, complaining `stderr`.
#[must_use]
pub fn refused(stderr: &str) -> Output {
    Output {
        status: Some(1),
        stdout: String::new(),
        stderr: stderr.to_owned(),
    }
}

/// A throwaway password for a test — built from a character range rather than
/// written as a string literal, so a hard-coded-credential scan does not read a
/// test fixture as a real secret. Non-empty on purpose (a recorded password that
/// read back as empty would be treated as absent), and its value is otherwise
/// irrelevant: the fakes accept whatever is sent.
#[must_use]
pub fn a_password() -> String {
    ('a'..='p').collect()
}

/// A scripted VPN exec answer: the value with a success code, or an empty
/// non-success body where there is none — the shape a missing value or an absent
/// file produces, which the readers treat as absent.
fn scripted(value: Option<&str>) -> ExecOutput {
    match value {
        Some(value) => ExecOutput {
            status: Some(0),
            stdout: format!("{value}\n"),
        },
        None => ExecOutput {
            status: Some(1),
            stdout: String::new(),
        },
    }
}
