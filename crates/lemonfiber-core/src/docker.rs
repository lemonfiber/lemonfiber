//! Making sense of what the engine reports.
//!
//! The port returns containers; this module turns them into the state a surface
//! renders — correlating containers back to the services that declared them,
//! and deciding what "started" actually means.
//!
//! Nothing here talks to anything. It is a function of a manifest, a plan and a
//! listing, which is what lets every state a service can be in be exercised
//! without a daemon, an image, or a running stack.
//!
//! The vocabulary is deliberately larger than the engine's. "Up" is an
//! ambiguity: a container whose process exists tells you nothing about whether
//! the application inside it is answering, and an operator told "started" who
//! then gets connection refused has been lied to.

use std::collections::BTreeMap;

use lemonfiber_manifest::Manifest;

// `Service` exposes a criticality as a public field, so the type has to be nameable by
// whoever reads one. It is the manifest's, re-exported here rather than left reachable
// only through a crate a consumer may not depend on.
pub use lemonfiber_manifest::Criticality;
use serde::Serialize;

use crate::ports::docker::{Container, Health, Lifecycle};

/// What one service is actually doing.
///
/// Ordered from worst to best, so a form's condition is the minimum across its
/// services and needs no comparison table. The declaration order is therefore
/// load-bearing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Exited without being asked to, and is not coming back on its own.
    Failed,
    /// Exiting and restarting repeatedly.
    CrashLooping,
    /// Running, and its own probe says it is not working.
    Unhealthy,
    /// No container exists.
    Absent,
    /// A container exists and is not running.
    Stopped,
    /// Running, and still inside its probe's start period.
    Starting,
    /// Running, and declaring no probe by which to know more.
    ///
    /// Distinct from healthy on purpose. A service that cannot be asked is not
    /// a service that answered, and rendering the two identically is how a
    /// dashboard comes to be believed about something it never checked.
    Running,
    /// Running and passing its own probe.
    Healthy,
    /// The operating system owns this one, not lemonfiber.
    HostManaged,
}

impl State {
    /// Whether this state is one that starting up is still waiting on.
    ///
    /// Settled does not mean well. A service that is crash-looping has settled
    /// into crash-looping, and waiting longer will not improve it.
    #[must_use]
    pub const fn settled(self) -> bool {
        !matches!(self, Self::Starting | Self::Absent | Self::Stopped)
    }

    /// Whether this state is one an operator needs to do something about.
    #[must_use]
    pub const fn wants_attention(self) -> bool {
        matches!(self, Self::Failed | Self::CrashLooping | Self::Unhealthy)
    }

    /// Whether lemonfiber has anything here to stop.
    ///
    /// `Absent` and `Stopped` have nothing to stop, and `Failed` has already
    /// stopped itself. A crash-looping service does have something to stop, and
    /// is the case that most wants stopping. `HostManaged` has something running
    /// too, but it is the operating system's and not lemonfiber's — which is the
    /// whole of what leaving a native Jellyfin alone comes to.
    #[must_use]
    pub const fn stoppable(self) -> bool {
        !matches!(
            self,
            Self::Absent | Self::Stopped | Self::Failed | Self::HostManaged
        )
    }
}

/// One service, as it stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct Service {
    /// The service's identifier, which is also its Compose service name.
    pub id: String,
    /// What it is called in front of an operator.
    pub name: String,
    /// The profile that declared it.
    pub profile: String,
    /// What it is doing.
    pub state: State,
    /// How much its absence costs, so a summary can weigh it.
    pub criticality: Criticality,
    /// The services it needs before it can work, as the manifest declares them.
    /// Carried so a failure can be attributed to the thing underneath it rather
    /// than counted as one more independent thing wrong.
    pub depends_on: Vec<String>,
    /// How it exited, where it has exited.
    pub exit: Option<i32>,
}

/// What a whole set of services amounts to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Condition {
    /// Nothing is running.
    Inactive,
    /// Running, with at least one service in a state that wants attention.
    Degraded,
    /// Some services are up and others are not.
    Partial,
    /// Everything expected is up and answering.
    Active,
}

/// Read one container's state, given whether the service declares a probe.
///
/// A container reaching `running` says nothing about whether the application
/// inside it has finished starting, which is why a probe's verdict outranks the
/// process existing. Where there is no probe, `Running` is the strongest claim
/// available and is reported as exactly that.
fn read(container: &Container) -> State {
    match container.lifecycle {
        Lifecycle::Running => match container.health {
            Health::Starting => State::Starting,
            Health::Healthy => State::Healthy,
            Health::Unhealthy => State::Unhealthy,
            Health::None => State::Running,
        },
        // Restarting is the engine's word for both a deliberate restart and a
        // container failing on a loop. Told apart by nothing else the engine
        // offers, the safer reading is the one that shows the operator
        // something rather than the one that hides it behind `starting`.
        Lifecycle::Restarting => State::CrashLooping,
        // Created has never run; paused exists and is not serving. Both are a
        // container that is there and doing nothing, which is what stopped
        // means — a paused container's last health verdict is a claim about a
        // container that can no longer answer.
        Lifecycle::Created | Lifecycle::Paused => State::Stopped,
        // A clean exit is a service that was stopped; any other is one that
        // fell over. An engine that has forgotten the code is not evidence of
        // either, so it is reported as merely stopped.
        Lifecycle::Exited | Lifecycle::Dead | Lifecycle::Removing => match container.exit {
            Some(0) | None => State::Stopped,
            Some(_) => State::Failed,
        },
    }
}

/// What every service in the named profiles is doing.
///
/// Services are taken from the manifest rather than from the listing, so a
/// service that was never started is reported as absent rather than omitted —
/// which is the difference between a dashboard saying nothing is wrong and one
/// saying nothing is there.
#[must_use]
pub fn survey(manifest: &Manifest, profiles: &[String], containers: &[Container]) -> Vec<Service> {
    let found: BTreeMap<&str, &Container> = containers
        .iter()
        .map(|container| (container.service.as_str(), container))
        .collect();

    let mut services: Vec<Service> = manifest
        .services
        .iter()
        .filter(|service| profiles.iter().any(|profile| profile == &service.profile))
        .map(|service| Service {
            id: service.id.clone(),
            name: service.name.clone(),
            profile: service.profile.clone(),
            state: if service.host_managed {
                State::HostManaged
            } else {
                found
                    .get(service.id.as_str())
                    .map_or(State::Absent, |c| read(c))
            },
            criticality: service.criticality,
            depends_on: service.depends_on.clone(),
            exit: found.get(service.id.as_str()).and_then(|c| c.exit),
        })
        .collect();

    // Worst first, then by name. An operator reading top to bottom meets the
    // thing that needs them before the nineteen things that do not.
    services.sort_by(|left, right| {
        left.state
            .cmp(&right.state)
            .then_with(|| left.id.cmp(&right.id))
    });
    services
}

/// What a set of services amounts to, as one word.
#[must_use]
pub fn condition(services: &[Service]) -> Condition {
    // Host-managed services are not lemonfiber's to start, so counting them
    // would make a form permanently partial through no fault of the operator.
    let ours: Vec<&Service> = services
        .iter()
        .filter(|service| service.state != State::HostManaged)
        .collect();

    // Nothing is up when every service is either absent or stopped: a stack the
    // operator stopped is not "partly up", it is down with its containers kept.
    if ours.is_empty()
        || ours
            .iter()
            .all(|service| matches!(service.state, State::Absent | State::Stopped))
    {
        return Condition::Inactive;
    }
    if ours.iter().any(|service| service.state.wants_attention()) {
        return Condition::Degraded;
    }
    if ours
        .iter()
        .all(|service| matches!(service.state, State::Healthy | State::Running))
    {
        return Condition::Active;
    }
    Condition::Partial
}

/// The order to stop these in: whatever depends on a service goes down before it does.
///
/// A torrent client shares the tunnel's network namespace, so the moment the tunnel
/// stops the client has no network at all — mid-write, mid-announce, with peers it
/// cannot tell. Stopping the client first costs a few seconds and leaves it able to
/// shut down the way it knows how.
///
/// Read from each service's own `depends_on` rather than from a rule about VPNs. The
/// tunnel is the case that matters on this stack, but "stop a thing before the thing
/// it needs" is the general shape, and a stack that adds another such pair gets the
/// same treatment without lemonfiber learning about it.
///
/// Ties are broken by name so the same request always produces the same command.
///
/// Services that depend on each other in a circle cannot be ordered, and are emitted
/// as they stand: refusing to stop them would be worse than stopping them in an order
/// nobody can fault, since there is no correct one.
#[must_use]
pub fn stopping_order(running: &[Service], stopping: &[String]) -> Vec<String> {
    let depends = |dependent: &str, on: &str| {
        running
            .iter()
            .any(|service| service.id == dependent && service.depends_on.iter().any(|id| id == on))
    };

    let mut left: Vec<String> = stopping.to_vec();
    left.sort();
    let mut order = Vec::with_capacity(left.len());

    while !left.is_empty() {
        let (ready, waiting): (Vec<String>, Vec<String>) = left
            .iter()
            .cloned()
            .partition(|id| !left.iter().any(|other| depends(other, id)));

        if ready.is_empty() {
            // A circle. There is no order that satisfies it, so the remainder goes as
            // it stands rather than the whole stop being refused over it.
            order.extend(waiting);
            return order;
        }
        order.extend(ready);
        left = waiting;
    }
    order
}

/// The services that starting is still waiting on.
#[must_use]
pub fn unsettled(services: &[Service]) -> Vec<&Service> {
    services
        .iter()
        .filter(|service| service.state != State::HostManaged)
        .filter(|service| !service.state.settled())
        .collect()
}

#[cfg(test)]
mod tests {
    use lemonfiber_manifest::{Criticality, Manifest};

    use super::{condition, read, stopping_order, survey, unsettled, Condition, Service, State};
    use crate::ports::docker::{Container, Health, Lifecycle};

    const STACK: &str = include_str!("../../../assets/media-stack/stack.toml");

    fn manifest() -> Option<Manifest> {
        Manifest::from_toml(STACK).ok()
    }

    /// A container in whatever condition a test needs.
    fn container(service: &str, lifecycle: Lifecycle, health: Health) -> Container {
        Container {
            id: format!("id-{service}"),
            project: "lemonfiber".to_owned(),
            service: service.to_owned(),
            lifecycle,
            health,
            exit: None,
        }
    }

    #[test]
    fn a_probe_s_verdict_outranks_the_process_existing() {
        for (health, expected) in [
            (Health::Starting, State::Starting),
            (Health::Healthy, State::Healthy),
            (Health::Unhealthy, State::Unhealthy),
            (Health::None, State::Running),
        ] {
            let running = container("sonarr", Lifecycle::Running, health);
            assert_eq!(read(&running), expected, "{health:?}");
        }
    }

    /// One service depending on another, for the ordering tests.
    fn depending(id: &str, on: &[&str]) -> Service {
        Service {
            id: id.to_owned(),
            name: id.to_owned(),
            profile: "torrent".to_owned(),
            state: State::Healthy,
            criticality: Criticality::Core,
            depends_on: on.iter().map(|id| (*id).to_owned()).collect(),
            exit: None,
        }
    }

    /// The case the requirement is about: the client shares the tunnel's network, so
    /// the tunnel going first would take the client's network out from under it.
    #[test]
    fn the_tunnel_goes_down_after_what_is_using_its_network() {
        let running = vec![
            depending("gluetun", &[]),
            depending("qbittorrent", &["gluetun"]),
        ];
        assert_eq!(
            stopping_order(&running, &["gluetun".to_owned(), "qbittorrent".to_owned()]),
            vec!["qbittorrent".to_owned(), "gluetun".to_owned()],
            "named the other way round, and still stopped in this one"
        );
    }

    /// Read from what each service declares rather than from a rule about VPNs, so a
    /// stack that adds another such pair gets the same treatment.
    #[test]
    fn anything_depended_on_goes_last_whatever_it_is() {
        let running = vec![
            depending("under", &[]),
            depending("over", &["under"]),
            depending("above", &["over"]),
        ];
        assert_eq!(
            stopping_order(
                &running,
                &["under".to_owned(), "over".to_owned(), "above".to_owned()]
            ),
            vec!["above".to_owned(), "over".to_owned(), "under".to_owned()],
            "each one goes before the thing it needs"
        );
    }

    /// The same request must produce the same command, or a golden file tests nothing.
    #[test]
    fn services_that_do_not_depend_on_each_other_are_ordered_by_name() {
        let running = vec![
            depending("sonarr", &[]),
            depending("radarr", &[]),
            depending("bazarr", &[]),
        ];
        assert_eq!(
            stopping_order(
                &running,
                &[
                    "sonarr".to_owned(),
                    "bazarr".to_owned(),
                    "radarr".to_owned()
                ]
            ),
            vec![
                "bazarr".to_owned(),
                "radarr".to_owned(),
                "sonarr".to_owned()
            ]
        );
    }

    /// A circle has no order that satisfies it. Stopping them in one nobody can fault
    /// beats refusing to stop them at all.
    #[test]
    fn services_that_need_each_other_are_still_stopped() {
        let running = vec![depending("one", &["other"]), depending("other", &["one"])];
        let order = stopping_order(&running, &["one".to_owned(), "other".to_owned()]);
        assert_eq!(order.len(), 2, "both are stopped: {order:?}");
        assert!(order.contains(&"one".to_owned()) && order.contains(&"other".to_owned()));
    }

    /// A service depending on something that is staying up is not held back by it.
    #[test]
    fn a_dependency_that_is_not_being_stopped_does_not_hold_anything_back() {
        let running = vec![
            depending("gluetun", &[]),
            depending("qbittorrent", &["gluetun"]),
        ];
        assert_eq!(
            stopping_order(&running, &["qbittorrent".to_owned()]),
            vec!["qbittorrent".to_owned()],
            "only what was asked for is stopped, and nothing waits on the rest"
        );
    }

    #[test]
    fn a_service_that_cannot_be_asked_is_not_a_service_that_answered() {
        let unprobed = read(&container("sonarr", Lifecycle::Running, Health::None));
        assert_ne!(
            unprobed,
            State::Healthy,
            "running without a probe must not be rendered as passing one"
        );
        assert_eq!(unprobed, State::Running);
    }

    #[test]
    fn stopping_on_purpose_and_falling_over_are_told_apart() {
        let mut stopped = container("sonarr", Lifecycle::Exited, Health::None);
        stopped.exit = Some(0);
        assert_eq!(read(&stopped), State::Stopped);

        let mut killed = container("sonarr", Lifecycle::Exited, Health::None);
        killed.exit = Some(137);
        assert_eq!(read(&killed), State::Failed);

        let forgotten = container("sonarr", Lifecycle::Exited, Health::None);
        assert_eq!(
            read(&forgotten),
            State::Stopped,
            "an engine that forgot the code is not evidence of a fault"
        );
    }

    #[test]
    fn every_lifecycle_the_engine_reports_has_a_state() {
        for (lifecycle, expected) in [
            (Lifecycle::Created, State::Stopped),
            (Lifecycle::Running, State::Running),
            (Lifecycle::Paused, State::Stopped),
            (Lifecycle::Restarting, State::CrashLooping),
            (Lifecycle::Exited, State::Stopped),
            (Lifecycle::Removing, State::Stopped),
            (Lifecycle::Dead, State::Stopped),
        ] {
            let found = read(&container("sonarr", lifecycle, Health::None));
            assert_eq!(found, expected, "{lifecycle:?}");
        }
    }

    #[test]
    fn a_looping_container_is_reported_as_looping_rather_than_as_starting() {
        let looping = read(&container("sonarr", Lifecycle::Restarting, Health::None));
        assert_eq!(looping, State::CrashLooping);
        assert!(looping.wants_attention());
        assert!(
            looping.settled(),
            "waiting longer does not improve a crash loop"
        );
    }

    #[test]
    fn what_starting_is_still_waiting_on_is_only_what_may_yet_change() {
        assert!(!State::Starting.settled());
        assert!(!State::Absent.settled());
        assert!(!State::Stopped.settled());
        for settled in [
            State::Healthy,
            State::Running,
            State::Unhealthy,
            State::Failed,
            State::CrashLooping,
            State::HostManaged,
        ] {
            assert!(settled.settled(), "{settled:?}");
        }
    }

    #[test]
    fn only_the_states_an_operator_must_act_on_ask_for_attention() {
        for wanting in [State::Failed, State::CrashLooping, State::Unhealthy] {
            assert!(wanting.wants_attention(), "{wanting:?}");
        }
        for quiet in [
            State::Healthy,
            State::Running,
            State::Starting,
            State::Absent,
            State::Stopped,
            State::HostManaged,
        ] {
            assert!(!quiet.wants_attention(), "{quiet:?}");
        }
    }

    #[test]
    fn a_service_that_was_never_started_is_absent_rather_than_missing_from_the_report() {
        let profiles = ["media".to_owned()];
        let surveyed = manifest().map(|manifest| survey(&manifest, &profiles, &[]));

        assert_eq!(
            surveyed.as_ref().map(|services| services
                .iter()
                .all(|service| service.state == State::Absent)),
            Some(true),
            "nothing running is not the same as nothing declared"
        );
        assert_eq!(
            surveyed.map(|services| services.is_empty()),
            Some(false),
            "the media profile declares services"
        );
    }

    #[test]
    fn a_survey_covers_the_named_profiles_and_nothing_else() {
        let profiles = ["media".to_owned()];
        let surveyed = manifest()
            .map(|manifest| survey(&manifest, &profiles, &[]))
            .unwrap_or_default();

        assert!(!surveyed.is_empty());
        assert!(
            surveyed.iter().all(|service| service.profile == "media"),
            "a survey of one profile must not report another's services"
        );
    }

    #[test]
    fn the_worst_state_is_reported_first() {
        let profiles = ["media".to_owned()];
        let broken = "jellyfin";
        let containers: Vec<Container> = MEDIA
            .iter()
            .map(|id| {
                let lifecycle = if *id == broken {
                    Lifecycle::Restarting
                } else {
                    Lifecycle::Running
                };
                container(id, lifecycle, Health::Healthy)
            })
            .collect();

        let surveyed = manifest()
            .map(|manifest| survey(&manifest, &profiles, &containers))
            .unwrap_or_default();
        assert_eq!(
            surveyed.first().map(|service| service.id.clone()),
            Some(broken.to_owned()),
            "the thing that needs the operator comes before the things that do not"
        );
    }

    /// Everything the `media` profile declares.
    const MEDIA: [&str; 4] = [
        "jellyfin",
        "seerr",
        "calibre-web-automated",
        "audiobookshelf",
    ];

    /// A stack where the operating system owns one of the services.
    ///
    /// Inline, because the stack this binary ships has no host-managed service
    /// and the rule still has to be proven. Transcoding on some hardware needs
    /// Jellyfin outside a container, and lemonfiber must not try to start it.
    const NATIVE: &str = r#"
schema_version = 1
stack_version = "1.0.0"
min_cli_version = "0.1.0"

[[profile]]
id = "media"
name = "Library"
description = "Serving what you have"

[[service]]
id = "jellyfin"
name = "Jellyfin"
profile = "media"
image = "jellyfin/jellyfin"
tag = "10.10.3"
criticality = "core"
license = "GPL-2.0-only"
upstream = "https://jellyfin.org"
last_release = "2026-01-01"
describes = "Plays what you own"
without_it = "Nothing plays"
host_managed = true

[[service]]
id = "seerr"
name = "Seerr"
profile = "media"
image = "fallenbagel/jellyseerr"
tag = "2.1.0"
criticality = "important"
license = "MIT"
upstream = "https://github.com/fallenbagel/jellyseerr"
last_release = "2026-01-01"
describes = "Takes requests"
without_it = "Nobody can ask for anything"

[[form]]
id = "library"
name = "Library"
description = "Serve what exists"
profiles = ["media"]
"#;

    #[test]
    fn a_service_the_operating_system_owns_is_never_reported_as_something_to_start() {
        let profiles = ["media".to_owned()];
        // The engine knows nothing about it, which for any other service would
        // mean absent — and absent is an invitation to start it.
        let surveyed = Manifest::from_toml(NATIVE)
            .ok()
            .map(|manifest| survey(&manifest, &profiles, &[]))
            .unwrap_or_default();

        assert_eq!(
            surveyed
                .iter()
                .map(|service| (service.id.clone(), service.state))
                .collect::<Vec<_>>(),
            vec![
                ("seerr".to_owned(), State::Absent),
                ("jellyfin".to_owned(), State::HostManaged),
            ],
            "the one lemonfiber does not own sorts last, because it wants nothing"
        );
        assert_eq!(
            condition(&surveyed),
            Condition::Inactive,
            "one host-managed service and one absent service is not a running stack"
        );
        assert!(unsettled(&surveyed).iter().all(|s| s.id != "jellyfin"));
    }

    /// A survey of the media profile with every service in one state.
    fn media(state: Lifecycle, health: Health) -> Vec<super::Service> {
        let profiles = ["media".to_owned()];
        let containers: Vec<Container> = MEDIA
            .iter()
            .map(|id| container(id, state, health))
            .collect();
        manifest()
            .map(|manifest| survey(&manifest, &profiles, &containers))
            .unwrap_or_default()
    }

    #[test]
    fn a_form_is_only_active_when_everything_it_needs_is_up() {
        assert_eq!(
            condition(&media(Lifecycle::Running, Health::Healthy)),
            Condition::Active
        );
        assert_eq!(
            condition(&media(Lifecycle::Running, Health::None)),
            Condition::Active,
            "a stack whose services declare no probe can still be up"
        );
    }

    #[test]
    fn one_failure_degrades_the_whole_rather_than_hiding_in_an_average() {
        let mut services = media(Lifecycle::Running, Health::Healthy);
        if let Some(first) = services.first_mut() {
            first.state = State::Unhealthy;
        }
        assert_eq!(condition(&services), Condition::Degraded);
    }

    #[test]
    fn nothing_running_is_inactive_and_something_missing_is_partial() {
        assert_eq!(condition(&[]), Condition::Inactive);

        let profiles = ["media".to_owned()];
        let absent = manifest()
            .map(|manifest| survey(&manifest, &profiles, &[]))
            .unwrap_or_default();
        assert_eq!(condition(&absent), Condition::Inactive);

        // A stack the operator stopped keeps its containers but runs nothing, so
        // it reads as inactive rather than as partly up.
        let stopped: Vec<super::Service> = media(Lifecycle::Running, Health::Healthy)
            .iter()
            .map(|service| super::Service {
                state: State::Stopped,
                ..service.clone()
            })
            .collect();
        assert_eq!(condition(&stopped), Condition::Inactive);

        let mut partial = media(Lifecycle::Running, Health::Healthy);
        if let Some(first) = partial.first_mut() {
            first.state = State::Absent;
        }
        assert_eq!(condition(&partial), Condition::Partial);

        let mut starting = media(Lifecycle::Running, Health::Healthy);
        if let Some(first) = starting.first_mut() {
            first.state = State::Starting;
        }
        assert_eq!(condition(&starting), Condition::Partial);
    }

    #[test]
    fn a_service_the_operating_system_owns_is_not_counted_against_the_form() {
        let mut services = media(Lifecycle::Running, Health::Healthy);
        if let Some(first) = services.first_mut() {
            first.state = State::HostManaged;
        }
        assert_eq!(
            condition(&services),
            Condition::Active,
            "a form is not permanently partial because Jellyfin runs natively"
        );

        let host_only: Vec<super::Service> = services
            .iter()
            .map(|service| super::Service {
                state: State::HostManaged,
                ..service.clone()
            })
            .collect();
        assert_eq!(condition(&host_only), Condition::Inactive);
    }

    #[test]
    fn starting_waits_only_on_what_may_still_change() {
        let settled = media(Lifecycle::Running, Health::Healthy);
        assert!(unsettled(&settled).is_empty());

        let mut waiting = media(Lifecycle::Running, Health::Healthy);
        if let Some(first) = waiting.first_mut() {
            first.state = State::Starting;
        }
        assert_eq!(unsettled(&waiting).len(), 1);

        let mut looping = media(Lifecycle::Running, Health::Healthy);
        if let Some(first) = looping.first_mut() {
            first.state = State::CrashLooping;
        }
        assert!(
            unsettled(&looping).is_empty(),
            "a crash loop has settled, and waiting for it is waiting forever"
        );

        let mut host = media(Lifecycle::Running, Health::Healthy);
        if let Some(first) = host.first_mut() {
            first.state = State::HostManaged;
        }
        assert!(unsettled(&host).is_empty());
    }

    #[test]
    fn a_survey_carries_what_a_summary_needs_to_weigh_a_service() {
        let services = media(Lifecycle::Running, Health::Healthy);
        assert!(services
            .iter()
            .any(|service| service.criticality != Criticality::Optional));
        assert!(services.iter().all(|service| !service.name.is_empty()));
    }
}
