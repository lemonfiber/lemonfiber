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

use std::sync::Arc;

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
    /// How many listings to refuse before this engine is there at all.
    ///
    /// Neither of the two states the rest of this fake describes. On two of the four
    /// platforms the engine is a desktop application that has only just been asked to
    /// start itself: it answers nothing for the better part of a minute and then
    /// answers everything, which is the case the wait at a login exists for and the
    /// one an engine that never changes its mind cannot produce.
    refusals: usize,
    /// How many listings it has refused so far, counted apart from the tally above
    /// because they are different questions — whether the engine is there at all, and
    /// whether what it holds has settled — and a test about one must not spend the
    /// other's budget.
    refused: std::sync::atomic::AtomicUsize,
    /// How many listings to answer before this engine is gone. Absent means it never goes.
    answers: Option<usize>,
    /// How many listings it has answered so far, counted only where it goes.
    answered: std::sync::atomic::AtomicUsize,
    /// What `exec` answers a VPN probe with, where the test scripts one. Absent
    /// means `exec` has nothing to say and fails, as an engine asked for a
    /// container it does not know would.
    tunnel: Option<Tunnel>,
    /// What every command run inside a container answers, where a test scripts one.
    exec: Option<ExecOutput>,
    /// The questions it refuses as an engine that is not answering them.
    refusing: Vec<Question>,
    /// Whether it takes every question and never answers any.
    hangs: bool,
    /// The engine listed beside it and asked everything else, where it stands beside one.
    over: Option<Arc<dyn Engine>>,
}

/// A question an engine can be asked besides what it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Question {
    /// A command run inside a container.
    Exec,
    /// A sample of what each container uses.
    Stats,
    /// What the containers wrote.
    Logs,
}

/// What a [`Reporting`] engine answers a VPN exec with: a public address for the
/// gateway and for the client (told apart by the gateway's service id appearing in
/// the container) and a country. A `None` field answers as absent — the exit code a
/// missing value produces.
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
                    mounts: Vec::new(),
                    exit: None,
                })
                .collect(),
            reachable: true,
            ..Self::default()
        }
    }

    /// The same engine, with every container having exited with this code.
    ///
    /// A code rather than a verdict, because the engine does not say why a container
    /// exited: a stop the operator asked for and a service falling over can leave the
    /// same number behind, and telling them apart is the reading's job.
    #[must_use]
    pub fn exiting(mut self, code: i32) -> Self {
        for container in &mut self.containers {
            container.exit = Some(code);
        }
        self
    }

    /// The same engine, with these services answering on these host addresses.
    ///
    /// Named rather than derived from anything, because what a check about bindings
    /// is about is what the engine *says* — and a fake that worked the addresses out
    /// from the services would be answering the question the check exists to ask.
    #[must_use]
    pub fn publishing(self, published: &[(&str, &str, u16)]) -> Self {
        let forwarded: Vec<(&str, &str, u16, u16)> = published
            .iter()
            .map(|&(service, address, port)| (service, address, port, port))
            .collect();
        self.forwarding(&forwarded)
    }

    /// The same engine, with these services answering on these host addresses and ports,
    /// each forwarded to the port its container listens on, last.
    #[must_use]
    pub fn forwarding(mut self, forwarded: &[(&str, &str, u16, u16)]) -> Self {
        for container in &mut self.containers {
            container.published = forwarded
                .iter()
                .filter(|(service, _, _, _)| *service == container.service)
                .filter_map(|(_, address, port, private)| {
                    Some(Published {
                        address: address.parse().ok()?,
                        port: *port,
                        private: *private,
                    })
                })
                .collect();
        }
        self
    }

    /// The same containers, belonging to somebody else's Compose project.
    ///
    /// For the survey that reads what is already on a machine, where what matters is
    /// precisely that a container is *not* lemonfiber's.
    #[must_use]
    pub fn belonging_to(mut self, project: &str) -> Self {
        for container in &mut self.containers {
            project.clone_into(&mut container.project);
        }
        self
    }

    /// The same engine, also holding everything another one holds.
    ///
    /// For the machine that is running two Compose projects at once, which is the only
    /// way to tell a question asked about one project from the same question asked
    /// about the other.
    #[must_use]
    pub fn alongside(mut self, other: Self) -> Self {
        self.containers.extend(other.containers);
        self
    }

    /// The same containers, with the given host paths mounted into each.
    ///
    /// For the survey that asks where an existing setup keeps its data, which is the
    /// only way to tell whether a hardlink could reach between two of its directories.
    #[must_use]
    pub fn mounting(mut self, paths: &[std::path::PathBuf]) -> Self {
        for container in &mut self.containers {
            container.mounts = paths.to_vec();
        }
        self
    }

    /// The same engine, unsettled until it has been asked `listings` times.
    #[must_use]
    pub fn settling_after(mut self, listings: usize) -> Self {
        self.settles_after = Some(listings);
        self
    }

    /// The same engine, not there at all for its first `refusals` listings.
    ///
    /// What a login meets, and a different thing from both of the states beside it: a
    /// desktop engine is neither reachable nor permanently absent while it is starting
    /// itself, and a run that gave up on the first refusal would report every Mac in
    /// the world as having failed to come back.
    #[must_use]
    pub fn waking_after(mut self, refusals: usize) -> Self {
        self.refusals = refusals;
        self
    }

    /// The same engine, gone once it has answered `listings` listings.
    #[must_use]
    pub fn leaving_after(mut self, listings: usize) -> Self {
        self.answers = Some(listings);
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

    /// The same engine, with this line among what its containers wrote.
    #[must_use]
    pub fn with_line(mut self, line: LogLine) -> Self {
        self.said.push(line);
        self
    }

    /// The same engine, answering every command run inside a container with `output`.
    #[must_use]
    pub fn answering_exec(mut self, output: ExecOutput) -> Self {
        self.exec = Some(output);
        self
    }

    /// The same engine, refusing each of `questions` as an engine not answering them.
    #[must_use]
    pub fn refusing(mut self, questions: &[Question]) -> Self {
        self.refusing.extend_from_slice(questions);
        self
    }

    /// The same engine, listed beside what `engine` holds, `engine` answering every other
    /// question; what `engine` cannot list is listed as nothing.
    #[must_use]
    pub fn over(mut self, engine: Arc<dyn Engine>) -> Self {
        self.over = Some(engine);
        self
    }

    /// An engine that takes every question and never answers any.
    #[must_use]
    pub fn hanging() -> Self {
        Self {
            hangs: true,
            ..Self::default()
        }
    }

    /// An engine that is not there.
    #[must_use]
    pub fn absent() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Engine for Reporting {
    async fn list(&self, project: &str) -> Result<Vec<Container>, EngineFailure> {
        self.hang().await;
        let Some(engine) = &self.over else {
            return listed(self, project);
        };
        let mut running = engine.list(project).await.unwrap_or_default();
        running.extend(listed(self, project)?);
        Ok(running)
    }

    async fn exec(&self, container: &str, argv: &[String]) -> Result<ExecOutput, EngineFailure> {
        self.hang().await;
        if let Some(engine) = &self.over {
            return engine.exec(container, argv).await;
        }
        self.answers(Question::Exec)?;
        match &self.exec {
            Some(output) => Ok(output.clone()),
            None => executed(self, container, argv),
        }
    }

    async fn stats(&self, project: &str) -> Result<Receiver<(String, Stats)>, EngineFailure> {
        self.hang().await;
        if let Some(engine) = &self.over {
            return engine.stats(project).await;
        }
        self.answers(Question::Stats)?;
        let (_sender, receiver) = tokio::sync::mpsc::channel(1);
        Ok(receiver)
    }

    async fn logs(
        &self,
        project: &str,
        services: &[String],
        query: LogQuery,
    ) -> Result<Receiver<LogLine>, EngineFailure> {
        self.hang().await;
        if let Some(engine) = &self.over {
            return engine.logs(project, services, query).await;
        }
        self.answers(Question::Logs)?;
        logs(self, services).await
    }
}

impl Reporting {
    /// Never coming back, where this engine hangs.
    async fn hang(&self) {
        if self.hangs {
            std::future::pending::<()>().await;
        }
    }

    /// Refused as an engine not answering `question`, where it refuses that.
    fn answers(&self, question: Question) -> Result<(), EngineFailure> {
        if self.refusing.contains(&question) {
            return Err(EngineFailure::Unreachable {
                reason: "nothing is running here".to_owned(),
            });
        }
        Ok(())
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

/// The containers this fixture holds, with health settled once it has been asked enough.
/// The containers of one Compose project, as the engine would answer.
///
/// Narrowed to `project` because the real adapter narrows: it asks the daemon for
/// containers carrying that project's label, so a fake that answered with every
/// container it held would let a caller pass a proof it never actually made. The
/// containers a fake is built with belong to lemonfiber's own project unless a test
/// said otherwise, so this filter is invisible to every test that never mentioned
/// one.
fn listed(reporting: &Reporting, project: &str) -> Result<Vec<Container>, EngineFailure> {
    if !reporting.reachable {
        return Err(EngineFailure::Unreachable {
            reason: "no daemon here".to_owned(),
        });
    }

    if reporting.answers.is_some_and(|answers| {
        reporting
            .answered
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            >= answers
    }) {
        return Err(EngineFailure::Unreachable {
            reason: "the daemon has gone".to_owned(),
        });
    }

    // Refused before anything is counted against the settling budget, so an engine
    // that was still starting arrives with the same patience for its containers as
    // one that was there from the outset.
    if reporting
        .refused
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        < reporting.refusals
    {
        return Err(EngineFailure::Unreachable {
            reason: "the daemon is still starting".to_owned(),
        });
    }

    let asked = reporting
        .asked
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let settled = reporting.settles_after.is_some_and(|after| asked >= after);

    Ok(reporting
        .containers
        .iter()
        .filter(|container| container.project == project)
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

/// Which scripted answer a command inside a container gets.
fn executed(
    reporting: &Reporting,
    container: &str,
    argv: &[String],
) -> Result<ExecOutput, EngineFailure> {
    let Some(tunnel) = &reporting.tunnel else {
        return Err(EngineFailure::NoSuchContainer {
            name: container.to_owned(),
        });
    };
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

async fn logs(
    reporting: &Reporting,
    services: &[String],
) -> Result<Receiver<LogLine>, EngineFailure> {
    // Opening a stream means finding the containers first, so an engine
    // that cannot be reached refuses this as surely as it refuses a
    // listing. A fake that answered anyway would be a fake that made
    // the health gate's own failure path untestable.
    if !reporting.reachable {
        return Err(EngineFailure::Unreachable {
            reason: "no daemon here".to_owned(),
        });
    }

    let wanted: Vec<LogLine> = reporting
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

#[cfg(test)]
mod tests;
