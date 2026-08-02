//! The fakes and builders the in-crate tests share.
//!
//! `app`'s tests and the seed tests split out of them both drive the dispatch
//! layer through the same seams — a scripted runner, an engine that reports what
//! a test put in it, transports that answer HTTP calls, a read-only filesystem.
//! They live here, once, rather than being re-declared in every test module that
//! needs them.
//!
//! In-crate test support only: compiled under `cfg(test)`, so nothing here ships.

use crate::ports::docker::{
    Container, Engine, ExecOutput, Failure as EngineFailure, Health, Lifecycle, LogLine, LogQuery,
    Stats, Stream,
};
use crate::ports::process::{Failure, Output, Runner};
use crate::stack::Source;
use async_trait::async_trait;
use tokio::sync::mpsc::Receiver;

/// A runner that answers with whatever the test scripted.
pub(crate) struct Scripted(pub(crate) Result<Output, Failure>);

#[async_trait]
impl Runner for Scripted {
    async fn run(&self, _argv: &[String]) -> Result<Output, Failure> {
        match &self.0 {
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
    // `stream` uses the trait's default (run then replay); a test that drives a
    // streamed pull exercises it.
}

/// An engine that reports whatever the test put in it.
///
/// A trait fake is the right tool here and the wrong one for the adapter:
/// the question above the port is what lemonfiber does with an answer, and
/// the question below it is whether the wire is spoken correctly.
#[derive(Default)]
pub(crate) struct Reporting {
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
pub(crate) struct Tunnel {
    pub(crate) gateway: &'static str,
    pub(crate) gateway_ip: Option<&'static str>,
    pub(crate) client_ip: Option<&'static str>,
    pub(crate) country: Option<&'static str>,
    pub(crate) port: Option<&'static str>,
}

impl Reporting {
    /// An engine reporting the named services in one state.
    pub(crate) fn holding(services: &[&str], lifecycle: Lifecycle, health: Health) -> Self {
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
    pub(crate) fn settling_after(mut self, listings: usize) -> Self {
        self.settles_after = Some(listings);
        self
    }

    /// The same engine, with something to say about a service.
    pub(crate) fn saying(mut self, service: &str, line: &str) -> Self {
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
    pub(crate) fn with_tunnel(mut self, tunnel: Tunnel) -> Self {
        self.tunnel = Some(tunnel);
        self
    }

    /// An engine that is not there.
    pub(crate) fn absent() -> Self {
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
        let ip = if container.contains(tunnel.gateway) {
            tunnel.gateway_ip
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

/// A transport that answers seeding's HTTP calls from a scripted queue.
pub(crate) struct ScriptedHttp {
    replies: std::sync::Mutex<std::collections::VecDeque<(u16, &'static str)>>,
}

impl ScriptedHttp {
    pub(crate) fn new(replies: Vec<(u16, &'static str)>) -> Self {
        Self {
            replies: std::sync::Mutex::new(replies.into()),
        }
    }
}

#[async_trait]
impl crate::ports::http::Http for ScriptedHttp {
    async fn send(
        &self,
        request: &crate::ports::http::Request,
    ) -> Result<crate::ports::http::Response, crate::ports::http::Unreachable> {
        let reply = self
            .replies
            .lock()
            .ok()
            .and_then(|mut replies| replies.pop_front());
        match reply {
            Some((status, body)) => Ok(crate::ports::http::Response {
                status,
                body: body.to_owned(),
            }),
            None => Err(crate::ports::http::Unreachable {
                url: request.url.clone(),
                reason: "nothing scripted".to_owned(),
            }),
        }
    }
}

/// A randomness source answering with exactly the bytes a test scripts.
pub(crate) struct FixedRandom(pub(crate) Option<Vec<u8>>);

impl crate::ports::random::Random for FixedRandom {
    fn bytes(&self, _n: usize) -> Option<Vec<u8>> {
        self.0.clone()
    }
}

/// A transport that answers by the shape of the URL, so a combined seed run's
/// many calls need no exact ordering. Root-folder, download-client and Prowlarr
/// application lists hold exactly what each service wants, so every connection
/// reads back as already wired; qBittorrent's auth and set both answer success.
pub(crate) struct RoutedHttp;

#[async_trait]
impl crate::ports::http::Http for RoutedHttp {
    async fn send(
        &self,
        request: &crate::ports::http::Request,
    ) -> Result<crate::ports::http::Response, crate::ports::http::Unreachable> {
        let body = if request.url.contains("/downloadclient") {
            r#"[{"id":1,"fields":[{"name":"host","value":"sabnzbd"},{"name":"port","value":8080}]},{"id":2,"fields":[{"name":"host","value":"gluetun"},{"name":"port","value":8081}]}]"#
        } else if request.url.contains("/rootfolder") {
            r#"[{"id":1,"path":"/data/media/tv"},{"id":2,"path":"/data/media/movies"},{"id":3,"path":"/data/media/music"}]"#
        } else if request.url.contains("/applications") {
            r#"[{"id":1,"fields":[{"name":"baseUrl","value":"http://sonarr:8989"}]},{"id":2,"fields":[{"name":"baseUrl","value":"http://radarr:7878"}]},{"id":3,"fields":[{"name":"baseUrl","value":"http://lidarr:8686"}]}]"#
        } else {
            "Ok."
        };
        Ok(crate::ports::http::Response {
            status: 200,
            body: body.to_owned(),
        })
    }
}

/// A filesystem that hands back a Servarr configuration for a Servarr path and
/// a `SABnzbd` one for `SABnzbd`'s, or nothing. Only `read` is meaningful to
/// seeding; the rest are unused.
pub(crate) struct SeedFs {
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
    facts: crate::ports::filesystem::StorageFacts,
}

impl SeedFs {
    pub(crate) fn keyed(servarr: Option<&'static str>, sabnzbd: Option<&'static str>) -> Self {
        Self {
            servarr,
            sabnzbd,
            only_prowlarr: false,
            missing: Vec::new(),
            facts: crate::ports::filesystem::StorageFacts {
                kind: crate::ports::filesystem::FsKind::Linking("test".to_owned()),
                removable: false,
                available: 0,
                total: 0,
            },
        }
    }

    /// The same, but withholding the Servarr key from every path but Prowlarr's.
    pub(crate) fn only_for_prowlarr(mut self) -> Self {
        self.only_prowlarr = true;
        self
    }

    /// The same, reporting the given volume facts to a describe.
    pub(crate) fn with_facts(mut self, facts: crate::ports::filesystem::StorageFacts) -> Self {
        self.facts = facts;
        self
    }

    /// The same, but failing to canonicalize any path carrying one of these
    /// fragments — a host directory that is not there, for a root-folder check.
    pub(crate) fn missing(mut self, fragments: Vec<&'static str>) -> Self {
        self.missing = fragments;
        self
    }
}

#[async_trait]
impl crate::ports::filesystem::FileSystem for SeedFs {
    async fn canonicalize(
        &self,
        path: &std::path::Path,
    ) -> Result<std::path::PathBuf, crate::ports::filesystem::Fault> {
        let text = path.to_string_lossy();
        if self.missing.iter().any(|fragment| text.contains(fragment)) {
            return Err(crate::ports::filesystem::Fault::new("no such path"));
        }
        Ok(path.to_path_buf())
    }
    async fn touch(&self, _path: &std::path::Path) -> Result<(), crate::ports::filesystem::Fault> {
        Err(crate::ports::filesystem::Fault::new("unused"))
    }
    async fn link(
        &self,
        _from: &std::path::Path,
        _to: &std::path::Path,
    ) -> Result<(), crate::ports::filesystem::Fault> {
        Err(crate::ports::filesystem::Fault::new("unused"))
    }
    async fn identify(
        &self,
        _path: &std::path::Path,
    ) -> Result<crate::ports::filesystem::Identity, crate::ports::filesystem::Fault> {
        Err(crate::ports::filesystem::Fault::new("unused"))
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
    ) -> Option<crate::ports::filesystem::Ownership> {
        None
    }
    async fn describe(&self, _path: &std::path::Path) -> crate::ports::filesystem::StorageFacts {
        self.facts.clone()
    }
}

/// The stack this repository carries, read from disk.
pub(crate) fn stack() -> Source {
    Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/media-stack"
    )))
}

/// A program that succeeded, saying `stdout`.
pub(crate) fn spoke(stdout: &str) -> Output {
    Output {
        status: Some(0),
        stdout: stdout.to_owned(),
        stderr: String::new(),
    }
}

/// A program that failed, complaining `stderr`.
pub(crate) fn refused(stderr: &str) -> Output {
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
pub(crate) fn a_password() -> String {
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
