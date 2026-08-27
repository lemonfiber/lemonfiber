//! Where the stack is **actually** listening, held against where it is meant to.
//!
//! Every other artefact in this product that says where a service listens says what
//! was *intended*: a compose file asks for an address, a manifest declares a tier, a
//! test reads both. None of them is evidence. A mapping edited by hand and applied,
//! an image whose defaults changed under an upgrade, a container started outside
//! Compose — each of those is a service listening somewhere nothing on disk says it
//! does, and a check that read the file would agree with the file and be wrong.
//!
//! So this asks the container engine what it published. That is the runtime's own
//! account of the sockets it is holding, and it is the only account in reach that
//! was not written by whoever wrote the intent.
//!
//! **Both families are read the same way.** The engine reports one entry per host
//! address, so a port published on `0.0.0.0` and on `::` is two entries and each is
//! held to the rule separately. A policy read on one family and not the other would
//! be absent on the one it did not read, which reads as enforced.
//!
//! **What it cannot see is written down rather than implied.** lemonfiber's own
//! surface is not a container, so it is not in this reading; where that one listens
//! is decided in one file and guarded from the shipped tree instead. And a service
//! the engine is not running publishes nothing, which is not the same as a service
//! bound correctly — a stack that is down is reported as one this could not be
//! established for.

use std::net::IpAddr;
use std::sync::Arc;

use async_trait::async_trait;
use lemonfiber_manifest::{Bind, Service};

use super::{Category, Check, Finding, Verdict};
use crate::error::{Code, Problem, Remedy, Severity, State};
use crate::platform::Environment;
use crate::ports::docker::{Container, Engine};

/// Raised when a service the stack calls admin answers somewhere off this machine.
pub const BEYOND_LOOPBACK: Code = Code::new("BIND-1");

/// Raised where a published port is reached without the host's own firewall rules
/// being consulted.
pub const AROUND_THE_FIREWALL: Code = Code::new("BIND-2");

/// Raised where an admin service answers off this machine and the operator has
/// written down that they meant it to.
pub const DELIBERATE: Code = Code::new("BIND-3");

/// The name this check's findings are given.
const CHECK: &str = "network.bindings";

/// What it is called on a report.
const TITLE: &str = "Where the stack is actually listening";

/// Whether the stack is listening where the policy says it may.
pub struct BindingsCheck {
    /// How the containers are asked what they published.
    engine: Arc<dyn Engine>,
    /// The Compose project they belong to.
    project: String,
    /// What the stack declares each service's tier to be.
    services: Vec<Service>,
    /// What this machine runs the engine as, which decides whether a published port
    /// is reached through the host's own firewall or around it.
    environment: Environment,
    /// The admin services the operator has said they meant to expose, each with why.
    exposed: Vec<(String, String)>,
}

impl BindingsCheck {
    /// A check over one project's containers, against one manifest's tiers.
    #[must_use]
    pub fn new(
        engine: Arc<dyn Engine>,
        project: String,
        services: &[Service],
        environment: Environment,
        exposed: &[(String, String)],
    ) -> Self {
        Self {
            engine,
            project,
            services: services.to_vec(),
            environment,
            exposed: exposed.to_vec(),
        }
    }

    /// Why the operator said this service is exposed, where they said so.
    fn said_so(&self, service: &str) -> Option<&str> {
        self.exposed
            .iter()
            .find(|(named, _)| named == service)
            .map(|(_, why)| why.as_str())
    }

    /// The tier the stack declares for a service, where it declares one.
    fn tier(&self, service: &str) -> Option<Bind> {
        self.services
            .iter()
            .find(|declared| declared.id == service)
            .and_then(|declared| declared.bind)
    }
}

/// Every address a container answers on that its tier does not allow.
///
/// The household tier allows every address, because reaching it from a phone is the
/// whole of what it is for. The admin tier allows this machine's own addresses and
/// nothing else, on either family. A service the manifest declares no tier for is
/// passed over rather than guessed at: a rule invented here would be a second opinion
/// about a question the stack already answers.
fn beyond(container: &Container, tier: Option<Bind>) -> Vec<String> {
    match tier {
        Some(Bind::Loopback) => container
            .published
            .iter()
            .filter(|published| !published.address.is_loopback())
            .map(|published| format!("{} on {}", published.port, wide(published.address)))
            .collect(),
        Some(Bind::Lan) | None => Vec::new(),
    }
}

/// An address as it is said back, with the wildcards named for what they mean.
///
/// `0.0.0.0` and `::` are the two an operator most needs told plainly: neither is an
/// address anything is at, and both mean *every interface this machine has*.
fn wide(address: IpAddr) -> String {
    if address.is_unspecified() {
        format!("{address}, which is every interface this machine has")
    } else {
        address.to_string()
    }
}

#[async_trait]
impl Check for BindingsCheck {
    fn category(&self) -> Category {
        Category::Network
    }

    async fn run(&self) -> Vec<Finding> {
        let Ok(containers) = self.engine.list(&self.project).await else {
            return vec![unverified()];
        };
        let running: Vec<&Container> = containers
            .iter()
            .filter(|container| !container.published.is_empty())
            .collect();
        if running.is_empty() {
            return vec![nothing_listening()];
        }
        let wrong: Vec<Finding> = running
            .iter()
            .flat_map(|container| {
                beyond(container, self.tier(&container.service))
                    .into_iter()
                    .map(|where_| match self.said_so(&container.service) {
                        Some(why) => acknowledged(&container.service, &where_, why),
                        None => violation(&container.service, &where_),
                    })
            })
            .collect();
        let mut found = if wrong.is_empty() {
            vec![kept(running.len())]
        } else {
            wrong
        };
        found.extend(around_the_firewall(self.environment, &running));
        found
    }
}

/// Where a published port is reached without the host's firewall being consulted.
///
/// Nothing here inspects a firewall. It cannot: the rules belong to whichever of
/// several tools the operator uses, and reading them would be a guess about which.
/// What it knows is the arrangement — the engine running directly on this machine
/// writes its own forwarding rules ahead of the ones a person adds, so a rule
/// written to close a port does not decide whether a published one answers.
///
/// That is a fact about the platform rather than about this machine's rules, so it
/// is said as one and only where it is true. Docker Desktop puts the engine behind
/// a virtual machine and forwards from a process the host's firewall does see, so
/// macOS, Windows and Desktop-on-Linux are not this case and are not warned about —
/// a warning that fired everywhere would be one nobody could act on.
fn around_the_firewall(environment: Environment, running: &[&Container]) -> Option<Finding> {
    if environment != Environment::LinuxNative {
        return None;
    }
    let mut reached: Vec<String> = running
        .iter()
        .flat_map(|container| {
            container
                .published
                .iter()
                .filter(|published| !published.address.is_loopback())
                .map(|published| format!("{} on {}", container.service, wide(published.address)))
        })
        .collect();
    if reached.is_empty() {
        return None;
    }
    reached.sort_unstable();
    reached.dedup();
    Some(bypassed(&reached))
}

/// What is said where a published port is answered around the host's own rules.
fn bypassed(reached: &[String]) -> Finding {
    let problem = Problem::new(
        AROUND_THE_FIREWALL,
        Severity::Warning,
        "A firewall rule on this machine may not apply to these".to_owned(),
        "The container engine runs directly on this machine here, and it writes its own \
         forwarding rules ahead of the ones you add. A rule you wrote to close one of these \
         ports is not what decides whether it answers, so a port you believe is shut may not \
         be. This is how publishing is meant to work and nothing is broken; it is worth \
         knowing because the thing you would reach for does not reach it.",
        Remedy::new("Narrow what the household tier is published on, which is what does decide")
            .with_detail(
                "LAN_BIND in your settings — set it to this machine's own address on \
                          your network rather than leaving it at every interface",
            ),
    )
    .or_try(Remedy::new(
        "Or leave it, if every device on this network is one you would let in anyway",
    ))
    .in_state(State::Guided)
    .with_detail(reached.join(", "));
    Finding::in_category(Category::Network, CHECK, TITLE, Verdict::Warn(problem))
}

/// One service answering somewhere its tier does not allow.
fn violation(service: &str, published: &str) -> Finding {
    let problem = Problem::new(
        BEYOND_LOOPBACK,
        Severity::Error,
        format!("{service} is reachable from your network"),
        "The stack declares this one as an admin service, which means it is meant to answer \
         this machine and nothing else. It can change how your stack works and most services \
         like it have weak or no password of their own, so anything on your network reaching \
         it is a way in.",
        Remedy::new("Publish it on this machine only, and apply the change")
            .with_detail(format!("127.0.0.1:{published}")),
    )
    .or_try(
        Remedy::new("Or, if you meant to expose it, say so once and say why").with_detail(format!(
            "{}={service}=<why> in your settings — the reason is what a diagnosis you send \
                 on will carry, and an entry without one is not read as an answer",
            crate::config::EXPOSED_KEY
        )),
    )
    .in_state(State::Guided)
    .with_detail(format!("it answers on {published}"));
    Finding::in_category(Category::Network, CHECK, TITLE, Verdict::Fail(problem)).about(service)
}

/// One service answering off this machine, which the operator said they meant.
///
/// Still reported, and no longer a failure. The exposure is real and does not stop
/// being real for having been agreed to — what changes is whose decision it is, and
/// a report that went silent would take the operator's own words out of the one
/// place anybody looking for them would find them. So their reason is quoted back:
/// a diagnosis somebody sends on carries why this was chosen, rather than an absence
/// the next reader has to guess about.
fn acknowledged(service: &str, published: &str, why: &str) -> Finding {
    let problem = Problem::new(
        DELIBERATE,
        Severity::Warning,
        format!("{service} is reachable from your network, and you said so"),
        format!(
            "The stack calls this one an admin service, so it is reported wherever it answers \
             off this machine. You have written down that this one is deliberate, and what you \
             said is: {why}"
        ),
        Remedy::new("Nothing, if that is still true"),
    )
    .or_try(Remedy::new(
        "Or take it out of the exposed list and publish it on this machine only",
    ))
    .in_state(State::Guided)
    .with_detail(format!("it answers on {published}"));
    Finding::in_category(Category::Network, CHECK, TITLE, Verdict::Warn(problem)).about(service)
}

/// Everything running answers where its tier allows.
fn kept(services: usize) -> Finding {
    Finding::in_category(
        Category::Network,
        CHECK,
        TITLE,
        Verdict::Pass {
            note: Some(format!(
                "{services} services publish a port, and every one of them answers where the \
                 stack says it may"
            )),
        },
    )
}

/// The engine is running and nothing published anything.
fn nothing_listening() -> Finding {
    Finding::in_category(
        Category::Network,
        CHECK,
        TITLE,
        Verdict::Skipped {
            reason: "nothing in this stack is publishing a port, so there is no binding to \
                     check — start it and ask again"
                .to_owned(),
        },
    )
}

/// The engine would not say.
fn unverified() -> Finding {
    Finding::in_category(
        Category::Network,
        CHECK,
        TITLE,
        Verdict::Unverified {
            reason: "the container engine would not say what it has published".to_owned(),
            remedy: Remedy::new("Start the container engine and ask again"),
        },
    )
}
