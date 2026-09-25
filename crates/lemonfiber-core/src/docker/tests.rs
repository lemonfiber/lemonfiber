use lemonfiber_manifest::{Criticality, Manifest};

use super::{
    condition, read, stopping_order, survey, undeclared, unsettled, Condition, Service, State,
    UNDESCRIBED,
};
use crate::config::Protocols;
use crate::ports::docker::{Container, Health, Lifecycle};

const STACK: &str = include_str!("../../../../assets/media-stack/stack.toml");

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
        published: Vec::new(),
        mounts: Vec::new(),
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
        describes: format!("what {id} is for"),
        profile: "torrent".to_owned(),
        forms: Vec::new(),
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
    let surveyed = manifest().map(|manifest| survey(&manifest, &profiles, &[], Protocols::both()));

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
        .map(|manifest| survey(&manifest, &profiles, &[], Protocols::both()))
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
        .map(|manifest| survey(&manifest, &profiles, &containers, Protocols::both()))
        .unwrap_or_default();
    assert_eq!(
        surveyed.first().map(|service| service.id.clone()),
        Some(broken.to_owned()),
        "the thing that needs the operator comes before the things that do not"
    );
}

/// Every service `library` holds is up, so each says `library` brought it, and a
/// service outside it says nothing brought it rather than guessing.
#[test]
fn a_surveyed_service_names_the_form_it_is_running_for() {
    let containers: Vec<Container> = MEDIA
        .iter()
        .map(|id| container(id, Lifecycle::Running, Health::Healthy))
        .collect();
    let profiles = ["media".to_owned(), "search".to_owned()];
    let surveyed = manifest()
        .map(|manifest| survey(&manifest, &profiles, &containers, Protocols::both()))
        .unwrap_or_default();

    assert!(!surveyed.is_empty());
    for service in &surveyed {
        let expected: &[&str] = if service.profile == "media" {
            &["library"]
        } else {
            &[]
        };
        assert_eq!(service.forms, expected, "{}", service.id);
    }
}

/// Everything the `media` profile declares.
const MEDIA: [&str; 5] = [
    "jellyfin",
    "seerr",
    "calibre-web-automated",
    "audiobookshelf",
    "navidrome",
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
        .map(|manifest| survey(&manifest, &profiles, &[], Protocols::both()))
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
        .map(|manifest| survey(&manifest, &profiles, &containers, Protocols::both()))
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
        .map(|manifest| survey(&manifest, &profiles, &[], Protocols::both()))
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

/// What a service is for travels with what it is doing, so anything reading a
/// survey can say it without going back to the manifest for a second answer.
///
/// Every one of them, rather than one: the field is required of every service the
/// manifest declares, and an assertion about the first would pass over a stack
/// that carried the description for one service and lost it for the rest.
#[test]
fn a_survey_carries_what_each_service_is_for() {
    let services = media(Lifecycle::Running, Health::Healthy);

    assert!(!services.is_empty(), "the stack was read");
    // One closure rather than a filter and a map. The naming half of a pair only
    // runs for what the filtering half let through, so on the passing run — the
    // one this test exists to have — it is a body nothing enters, and the
    // coverage gate counts it against this file.
    let silent: Vec<&str> = services
        .iter()
        .filter_map(|service| {
            service
                .describes
                .trim()
                .is_empty()
                .then_some(service.id.as_str())
        })
        .collect();
    assert!(
        silent.is_empty(),
        "these reach a surface with nothing to say about themselves: {silent:?}"
    );
    assert!(
        services
            .iter()
            .any(|service| service.describes.split_whitespace().count() > 2),
        "and the words are the stack's own, not a label: {services:?}"
    );
}

/// A container this stack never declared is reported, with the one honest thing
/// there is to say about it — that nothing here knows what it is for.
#[test]
fn a_container_the_stack_never_declared_is_shown_rather_than_hidden() {
    let containers = vec![
        container("sonarr", Lifecycle::Running, Health::Healthy),
        container("something-of-their-own", Lifecycle::Running, Health::None),
    ];
    let strangers = manifest()
        .map(|manifest| undeclared(&manifest, &containers))
        .unwrap_or_default();

    assert_eq!(
        strangers
            .iter()
            .map(|one| (one.id.as_str(), one.state))
            .collect::<Vec<_>>(),
        vec![("something-of-their-own", State::Running)],
        "the declared one is the survey's business, and this one is nobody else's"
    );
    assert!(
        strangers
            .first()
            .is_some_and(|one| one.describes == UNDESCRIBED),
        "an unknown description rather than an empty one: {strangers:?}"
    );
}

/// And the survey it sits beside is untouched by it, which is the half that keeps
/// a strange container out of everything that waits on, grades or stops a stack.
#[test]
fn a_container_the_stack_never_declared_is_no_part_of_the_survey() {
    let profiles = ["media".to_owned()];
    let containers = vec![container(
        "something-of-their-own",
        Lifecycle::Running,
        Health::None,
    )];
    let surveyed = manifest()
        .map(|manifest| survey(&manifest, &profiles, &containers, Protocols::both()))
        .unwrap_or_default();

    assert!(
        surveyed
            .iter()
            .all(|service| service.id != "something-of-their-own"),
        "{surveyed:?}"
    );
    assert_eq!(
        condition(&surveyed),
        Condition::Inactive,
        "a container nobody declared cannot make a stopped stack read as partly up"
    );
}

/// One scaled service is one thing an operator does not recognise, and the answer
/// has to be stable — a listing whose order follows whatever the engine happened
/// to say is one nobody can compare against the last time they looked.
#[test]
fn two_containers_of_one_strange_service_are_named_once_and_in_order() {
    let containers = vec![
        container("zeta", Lifecycle::Running, Health::None),
        container("alpha", Lifecycle::Running, Health::None),
        container("alpha", Lifecycle::Running, Health::None),
    ];
    let strangers = manifest()
        .map(|manifest| undeclared(&manifest, &containers))
        .unwrap_or_default();

    assert_eq!(
        strangers
            .iter()
            .map(|one| one.id.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
}
