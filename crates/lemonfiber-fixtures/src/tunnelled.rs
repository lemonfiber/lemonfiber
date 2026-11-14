//! A container engine that answers as a tunnel and a client behind it would.
//!
//! What it scripts is the VPN check's whole conversation: whether each container is up,
//! what address and country each reports, and how the tunnel's device answers being taken
//! down and put back, so the killswitch can be tested holding and leaking.

use async_trait::async_trait;
use lemonfiber_ports::docker::{
    Container, Engine, ExecOutput, Failure, Health, Lifecycle, LogLine, LogQuery, Stats,
};
use tokio::sync::mpsc::Receiver;

/// How one container should answer the check.
#[derive(Clone)]
pub struct Behavior {
    /// The compose service this behaviour is for.
    pub service: &'static str,
    /// Whether the container is up.
    pub running: bool,
    /// The public address it reports, or nothing where it reports none.
    pub address: Option<&'static str>,
    /// The country that address resolves to, where the test cares.
    pub country: Option<&'static str>,
    /// A different answer for a second address service, where the test is about
    /// the sources contradicting each other.
    pub second_opinion: Option<&'static str>,
    /// Whether an exec into it fails outright, rather than answering.
    pub exec_fails: bool,
}

impl Behavior {
    /// A container that is running, reporting this address where it is asked for one.
    #[must_use]
    pub fn up(service: &'static str, address: Option<&'static str>) -> Self {
        Self {
            service,
            running: true,
            address,
            country: None,
            second_opinion: None,
            exec_fails: false,
        }
    }
}

/// How the tunnel container answers being asked to put the device back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restore {
    /// The device comes back.
    Works,
    /// The command is refused.
    Fails,
    /// The command never answers at all.
    Silent,
}

/// How the tunnel container answers the killswitch test, and what the client
/// sees while the tunnel is down.
#[derive(Debug, Clone, Copy)]
pub struct Link {
    /// The device the default route names, or nothing where there is no route.
    pub device: Option<&'static str>,
    /// Whether `ip link set` succeeds.
    pub movable: bool,
    /// How putting the device back up is answered.
    pub restore: Restore,
    /// What the client's address probe answers while the tunnel is down. `None`
    /// is traffic that stopped, which is the killswitch holding.
    pub leaks_as: Option<&'static str>,
    /// Whether the client's probe never answers at all while the tunnel is down.
    ///
    /// A container whose network went away with the tunnel can leave an exec waiting
    /// with nothing to answer it, which is the shape of hang that outlasts a budget.
    pub probe_hangs: bool,
    /// How many seconds the route read takes, so a test can spend the budget before
    /// the tunnel is touched.
    pub route_takes: u64,
}

impl Link {
    /// A tunnel that drops and comes back, and a client whose traffic stops with
    /// it — the stack the killswitch is supposed to produce.
    #[must_use]
    pub const fn holding() -> Self {
        Self {
            device: Some("tun0"),
            movable: true,
            restore: Restore::Works,
            leaks_as: None,
            probe_hangs: false,
            route_takes: 0,
        }
    }
}

/// An engine that answers exec and list from a fixed script.
pub struct Fake {
    /// Whether the engine itself answers at all.
    pub reachable: bool,
    /// How each container answers, one entry per service.
    pub behaviors: Vec<Behavior>,
    /// How the killswitch probe is answered, where a test scripts it. Absent is
    /// an image with no `ip` in it.
    pub link: Option<Link>,
    /// Whether the tunnel device is down right now.
    pub dropped: std::sync::atomic::AtomicBool,
    /// Whether the tunnel device has been taken down at any point in this run.
    ///
    /// Apart from `dropped` because the two answer different questions. A check
    /// that took the tunnel away and put it back leaves `dropped` false, so a test
    /// asking whether anything was disturbed would read that as nothing having
    /// happened — which is exactly the disturbance a run that may not act must not
    /// have caused.
    pub disturbed: std::sync::atomic::AtomicBool,
}

impl Fake {
    /// An engine answering with these behaviours, in the order they are given.
    #[must_use]
    pub fn new(behaviors: Vec<Behavior>) -> Self {
        Self {
            reachable: true,
            behaviors,
            link: None,
            dropped: std::sync::atomic::AtomicBool::new(false),
            disturbed: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The same, answering the killswitch test as `link` says.
    #[must_use]
    pub fn linked(behaviors: Vec<Behavior>, link: Link) -> Self {
        Self {
            link: Some(link),
            ..Self::new(behaviors)
        }
    }

    /// Whether the tunnel device is currently down.
    #[must_use]
    pub fn is_dropped(&self) -> bool {
        self.dropped.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Whether the tunnel device was ever taken down, restored or not.
    #[must_use]
    pub fn was_dropped(&self) -> bool {
        self.disturbed.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// How long this `ip` command takes to answer, where the test slowed it down or
    /// stopped it answering at all.
    pub async fn dawdle(&self, argv: &[String]) {
        let Some(link) = self.link else { return };
        if argv.get(1).is_some_and(|arg| arg == "route") && link.route_takes > 0 {
            tokio::time::sleep(std::time::Duration::from_secs(link.route_takes)).await;
        }
        if argv.last().is_some_and(|arg| arg == "up") && link.restore == Restore::Silent {
            std::future::pending::<()>().await;
        }
    }

    /// The tunnel container's answer to an `ip` command.
    #[must_use]
    pub fn answer_ip(&self, argv: &[String]) -> ExecOutput {
        let Some(link) = self.link else {
            // An image with no `ip` in it — a fork this test cannot be run against.
            return ExecOutput {
                status: Some(127),
                stdout: String::new(),
            };
        };
        if argv.get(1).is_some_and(|arg| arg == "route") {
            // A dropped device carries no default route, which is also how the
            // check confirms the restore took.
            let device = if self.is_dropped() { None } else { link.device };
            return ExecOutput {
                status: Some(0),
                stdout: device.map_or_else(String::new, |device| {
                    format!("default via 10.8.0.1 dev {device} proto static")
                }),
            };
        }
        let up = argv.last().is_some_and(|arg| arg == "up");
        if !link.movable || (up && link.restore != Restore::Works) {
            return ExecOutput {
                status: Some(2),
                stdout: String::new(),
            };
        }
        self.dropped
            .store(!up, std::sync::atomic::Ordering::Relaxed);
        if !up {
            self.disturbed
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        ExecOutput {
            status: Some(0),
            stdout: String::new(),
        }
    }
}

#[async_trait]
impl Engine for Fake {
    async fn list(&self, _project: &str) -> Result<Vec<Container>, Failure> {
        if !self.reachable {
            return Err(Failure::Unreachable {
                reason: "no daemon".to_owned(),
            });
        }
        Ok(self
            .behaviors
            .iter()
            .map(|behavior| Container {
                id: format!("id-{}", behavior.service),
                project: "lemonfiber".to_owned(),
                service: behavior.service.to_owned(),
                lifecycle: if behavior.running {
                    Lifecycle::Running
                } else {
                    Lifecycle::Exited
                },
                health: Health::None,
                published: Vec::new(),
                mounts: Vec::new(),
                exit: None,
            })
            .collect())
    }

    async fn exec(&self, container: &str, argv: &[String]) -> Result<ExecOutput, Failure> {
        let behavior = self
            .behaviors
            .iter()
            .find(|behavior| format!("id-{}", behavior.service) == container)
            .ok_or_else(|| Failure::NoSuchContainer {
                name: container.to_owned(),
            })?;
        if behavior.exec_fails {
            return Err(Failure::NoSuchContainer {
                name: container.to_owned(),
            });
        }
        if argv.first().is_some_and(|arg| arg == "ip") {
            self.dawdle(argv).await;
            return Ok(self.answer_ip(argv));
        }
        let wants_country = argv.last().is_some_and(|arg| arg.ends_with("/country-iso"));
        if wants_country {
            return Ok(ExecOutput {
                status: Some(0),
                stdout: behavior.country.unwrap_or_default().to_owned(),
            });
        }
        // A second address service, where the test gave one a different answer —
        // which is what a cached or misconfigured echo looks like from here.
        let asked_second = argv
            .last()
            .is_some_and(|arg| arg.contains("second.example"));
        // A client cut off with the tunnel can leave the exec waiting rather than
        // answering, where the test asks for that.
        if self.is_dropped()
            && behavior.service != "gluetun"
            && self.link.is_some_and(|link| link.probe_hangs)
        {
            std::future::pending::<()>().await;
        }
        // While the tunnel is down the client answers with whatever the test says
        // still gets out — nothing at all, where the killswitch holds.
        let address = if self.is_dropped() && behavior.service != "gluetun" {
            self.link.and_then(|link| link.leaks_as)
        } else if asked_second {
            behavior.second_opinion.or(behavior.address)
        } else {
            behavior.address
        };
        match address {
            Some(address) => Ok(ExecOutput {
                status: Some(0),
                stdout: format!("{address}\n"),
            }),
            None => Ok(ExecOutput {
                status: Some(1),
                stdout: String::new(),
            }),
        }
    }

    async fn stats(&self, _project: &str) -> Result<Receiver<(String, Stats)>, Failure> {
        let (_sender, receiver) = tokio::sync::mpsc::channel(1);
        Ok(receiver)
    }

    async fn logs(
        &self,
        _project: &str,
        _services: &[String],
        _query: LogQuery,
    ) -> Result<Receiver<LogLine>, Failure> {
        let (_sender, receiver) = tokio::sync::mpsc::channel(1);
        Ok(receiver)
    }
}
