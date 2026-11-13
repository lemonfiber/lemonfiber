//! The machine a staged update runs on: its engine and its runner as one.
//!
//! A staged update stops the stack, captures it while nothing can be writing, and then
//! starts one service at a time, so the engine says nothing is running while the capture
//! is taken and that a service is immediately afterwards. Two fakes answering apart
//! could not be both.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use lemonfiber_ports::docker::{
    Container, Engine, ExecOutput, Failure as EngineFailure, Health, Lifecycle, LogLine, LogQuery,
    Stats,
};
use lemonfiber_ports::process::{Failure as RunFailure, Output, Runner};
use tokio::sync::mpsc::{channel, Receiver};

use crate::support::spoke;

/// How a service the run started comes back, once it has been started.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Coming {
    /// Running and answering its own probe.
    Answering,
    /// Running and saying it is not working.
    Unwell,
    /// Nothing appears at all, so the wait runs out.
    Never,
    /// Still inside its probe's start period for this many listings, then answering.
    Slowly(usize),
    /// The engine stops answering the moment something has been started.
    Silent,
}

/// The service one Compose invocation names, where it names one.
///
/// The fence is what tells a start aimed at one service from the whole-stack stop in
/// front of it and the whole-stack start behind it: only a narrowed invocation puts
/// names after `--`.
fn fenced(argv: &[String]) -> Option<String> {
    let at = argv.iter().position(|word| word == "--")?;
    argv.get(at + 1).cloned()
}

/// What the engine reports and what the runner did, kept in one place.
pub struct Machine {
    /// Every argument vector Compose was handed, in order.
    seen: Mutex<Vec<Vec<String>>>,
    /// The services started so far, in the order the run started them.
    started: Mutex<Vec<String>>,
    /// How many times the engine has been asked what is running once something was.
    asked: Mutex<usize>,
    /// How a started service comes back.
    coming: Coming,
    /// What Compose says to a start, where a test is about it refusing one.
    start: Mutex<Option<Result<Output, RunFailure>>>,
    /// What Compose says to the first whole-stack invocation — the stop in front of
    /// the capture — where a test is about it refusing that.
    stack: Mutex<Option<Result<Output, RunFailure>>>,
    /// What Compose says to the whole-stack start behind the run, where a test is
    /// about it refusing that.
    bringing_back: Mutex<Option<Result<Output, RunFailure>>>,
    /// What Compose says to the fetch in front of the stop, where a test is about it
    /// failing.
    fetch: Mutex<Option<Result<Output, RunFailure>>>,
    /// How many whole-stack invocations have been asked for.
    ///
    /// The stop in front of the capture is the first and the start that puts
    /// everything back is the last, so a test that wants one of them refused has to
    /// be able to say which. Refusing "the stack" without saying when would always
    /// refuse the stop, and the two leave the machine in opposite states.
    stack_actions: Mutex<usize>,
    /// What else is running until the run first acts on the whole stack, where a test
    /// reaches a service the run does not start.
    beside: Mutex<Option<Arc<dyn Engine>>>,
}

impl Machine {
    /// A machine whose started services come back the given way.
    #[must_use]
    pub fn coming(coming: Coming) -> Arc<Self> {
        Arc::new(Self {
            seen: Mutex::new(Vec::new()),
            started: Mutex::new(Vec::new()),
            asked: Mutex::new(0),
            coming,
            start: Mutex::new(None),
            stack: Mutex::new(None),
            bringing_back: Mutex::new(None),
            fetch: Mutex::new(None),
            stack_actions: Mutex::new(0),
            beside: Mutex::new(None),
        })
    }

    /// The same machine, with `running` listed beside what it starts until the run first
    /// acts on the whole stack.
    #[must_use]
    pub fn beside(self: Arc<Self>, running: Arc<dyn Engine>) -> Arc<Self> {
        if let Ok(mut beside) = self.beside.lock() {
            *beside = Some(running);
        }
        self
    }

    /// What is running beside what the run starts: nothing once the run has acted on the
    /// whole stack.
    async fn running_beside(&self, project: &str) -> Vec<Container> {
        let untouched = self.stack_actions.lock().is_ok_and(|acted| *acted == 0);
        let beside = self
            .beside
            .lock()
            .ok()
            .and_then(|beside| beside.clone())
            .filter(|_| untouched);
        match beside {
            Some(engine) => engine.list(project).await.unwrap_or_default(),
            None => Vec::new(),
        }
    }

    /// The same machine, with Compose failing the fetch in front of the stop.
    #[must_use]
    pub fn failing_the_fetch(self: Arc<Self>, said: Result<Output, RunFailure>) -> Arc<Self> {
        if let Ok(mut fetch) = self.fetch.lock() {
            *fetch = Some(said);
        }
        self
    }

    /// Every Compose invocation, in order, as the subcommand it ran and what it named.
    #[must_use]
    pub fn asked(&self) -> Vec<Vec<String>> {
        self.seen
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default()
    }

    /// The same machine, with Compose refusing the first start it is asked for.
    #[must_use]
    pub fn refusing(self: Arc<Self>, said: Result<Output, RunFailure>) -> Arc<Self> {
        if let Ok(mut start) = self.start.lock() {
            *start = Some(said);
        }
        self
    }

    /// The same machine, with Compose refusing the stop that comes before the capture.
    #[must_use]
    pub fn refusing_the_stack(self: Arc<Self>, said: Result<Output, RunFailure>) -> Arc<Self> {
        if let Ok(mut stack) = self.stack.lock() {
            *stack = Some(said);
        }
        self
    }

    /// The same machine, with Compose refusing the start that puts the stack back.
    ///
    /// The run has finished by then and every step of it has succeeded, so this is
    /// the stack failing to come back rather than the update failing.
    #[must_use]
    pub fn refusing_to_bring_it_back(
        self: Arc<Self>,
        said: Result<Output, RunFailure>,
    ) -> Arc<Self> {
        if let Ok(mut back) = self.bringing_back.lock() {
            *back = Some(said);
        }
        self
    }

    /// The last thing Compose was asked to do, as the words it was asked in.
    #[must_use]
    pub fn last(&self) -> Vec<String> {
        self.seen
            .lock()
            .ok()
            .and_then(|seen| seen.last().cloned())
            .unwrap_or_default()
    }

    /// The services this run has started, in order.
    #[must_use]
    pub fn started(&self) -> Vec<String> {
        self.started
            .lock()
            .map(|seen| seen.clone())
            .unwrap_or_default()
    }

    /// One container for a service the run has started.
    fn container(service: &str, health: Health) -> Container {
        Container {
            id: format!("id-{service}"),
            project: "lemonfiber".to_owned(),
            service: service.to_owned(),
            lifecycle: Lifecycle::Running,
            health,
            published: Vec::new(),
            mounts: Vec::new(),
            exit: None,
        }
    }

    /// How the services started so far answer their probes on this listing.
    fn health(&self, listings: usize) -> Health {
        match self.coming {
            Coming::Unwell => Health::Unhealthy,
            Coming::Slowly(after) if listings <= after => Health::Starting,
            _ => Health::Healthy,
        }
    }
}

#[async_trait]
impl Runner for Machine {
    async fn run(&self, argv: &[String]) -> Result<Output, RunFailure> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(argv.to_vec());
        }
        if argv.iter().any(|word| word == "pull") {
            return self
                .fetch
                .lock()
                .ok()
                .and_then(|mut said| said.take())
                .unwrap_or_else(|| Ok(spoke("")));
        }
        let Some(service) = fenced(argv) else {
            let nth = self.stack_actions.lock().map_or(1, |mut asked| {
                *asked += 1;
                *asked
            });
            let refusal = if nth == 1 {
                &self.stack
            } else {
                &self.bringing_back
            };
            return refusal
                .lock()
                .ok()
                .and_then(|mut said| said.take())
                .unwrap_or_else(|| Ok(spoke("")));
        };
        if let Some(said) = self.start.lock().ok().and_then(|mut start| start.take()) {
            return said;
        }
        if let Ok(mut started) = self.started.lock() {
            started.push(service);
        }
        Ok(spoke(""))
    }
}

#[async_trait]
impl Engine for Machine {
    async fn list(&self, project: &str) -> Result<Vec<Container>, EngineFailure> {
        // Counted only once something has been started, because what the count stands
        // for is how long a started service has been coming up. A listing taken while
        // the stack is down — the stop reading back what it stopped — is not a look at
        // anything starting.
        let started = self.started();
        if started.is_empty() {
            return Ok(self.running_beside(project).await);
        }
        let listings = self.asked.lock().map_or(0, |mut asked| {
            *asked += 1;
            *asked
        });
        if self.coming == Coming::Silent {
            return Err(EngineFailure::Unreachable {
                reason: "the daemon went away".to_owned(),
            });
        }
        if self.coming == Coming::Never {
            return Ok(Vec::new());
        }
        let health = self.health(listings);
        Ok(started
            .iter()
            .map(|service| Self::container(service, health))
            .collect())
    }

    async fn exec(&self, container: &str, _argv: &[String]) -> Result<ExecOutput, EngineFailure> {
        Err(EngineFailure::NoSuchContainer {
            name: container.to_owned(),
        })
    }

    async fn stats(&self, _project: &str) -> Result<Receiver<(String, Stats)>, EngineFailure> {
        let (_sender, receiver) = channel(1);
        Ok(receiver)
    }

    async fn logs(
        &self,
        _project: &str,
        _services: &[String],
        _query: LogQuery,
    ) -> Result<Receiver<LogLine>, EngineFailure> {
        let (_sender, receiver) = channel(1);
        Ok(receiver)
    }
}
