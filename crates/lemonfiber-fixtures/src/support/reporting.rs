//! The container engine these tests are driven through.
//!
//! A trait fake is the right tool here and the wrong one for the adapter: the
//! question above the port is what lemonfiber does with an answer, and the question
//! below it is whether the wire is spoken correctly.
//!
//! Its own file, because what it answers has grown past the one thing every other
//! fake beside it answers — a lifecycle, a health, a log stream, a command run
//! inside a container, and the addresses each container publishes are five
//! questions rather than one.

use async_trait::async_trait;
use lemonfiber_ports::docker::{
    Container, Engine, ExecOutput, Failure as EngineFailure, Health, Lifecycle, LogLine, LogQuery,
    Published, Stats, Stream,
};
use tokio::sync::mpsc::Receiver;

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
                    published: Vec::new(),
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

    /// The same engine, with these services answering on these host addresses.
    ///
    /// Named rather than derived from anything, because what a check about bindings
    /// is about is what the engine *says* — and a fake that worked the addresses out
    /// from the services would be answering the question the check exists to ask.
    #[must_use]
    pub fn publishing(mut self, published: &[(&str, &str, u16)]) -> Self {
        for container in &mut self.containers {
            container.published = published
                .iter()
                .filter(|(service, _, _)| *service == container.service)
                .filter_map(|(_, address, port)| {
                    Some(Published {
                        address: address.parse().ok()?,
                        port: *port,
                    })
                })
                .collect();
        }
        self
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

    /// The same engine, with something to say about a service at a given moment.
    ///
    /// The stamp is what the container itself claims, which is the only thing a
    /// reader can order several services by — so a test about ordering needs to set
    /// it, and one about content does not.
    #[must_use]
    pub fn saying_at(mut self, service: &str, at: &str, line: &str) -> Self {
        self.said.push(LogLine {
            service: service.to_owned(),
            stream: Stream::Stdout,
            at: Some(at.to_owned()),
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

/// A scripted VPN exec answer: the value with a success code, or an empty
/// non-success body where there is none — the shape a missing value or an absent
/// file produces, which the readers treat as absent.
pub(super) fn scripted(value: Option<&str>) -> ExecOutput {
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
