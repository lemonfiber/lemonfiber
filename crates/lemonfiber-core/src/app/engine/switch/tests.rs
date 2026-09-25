use super::{leaving, moved};
use crate::docker::{Criticality, Service, State};
use lemonfiber_manifest::Manifest;

const STACK: &str = include_str!("../../../../../../assets/media-stack/stack.toml");

fn service(id: &str, state: State) -> Service {
    Service {
        id: id.to_owned(),
        name: id.to_owned(),
        describes: format!("what {id} is for"),
        profile: "media".to_owned(),
        forms: Vec::new(),
        state,
        criticality: Criticality::Core,
        depends_on: Vec::new(),
        exit: None,
    }
}

/// The whole point of the verb: a service both shapes hold is not touched.
#[test]
fn a_service_the_new_shape_also_holds_is_kept_rather_than_restarted() {
    let running = vec![
        service("jellyfin", State::Healthy),
        service("qbittorrent", State::Healthy),
    ];
    let holds = vec!["jellyfin".to_owned(), "sonarr".to_owned()];

    let switched = moved(&running, &holds);

    assert_eq!(switched.kept, vec!["jellyfin".to_owned()]);
    assert_eq!(
        switched.stopped,
        vec!["qbittorrent".to_owned()],
        "only what fell outside the new closure is stopped"
    );
    assert_eq!(
        switched.started,
        vec!["sonarr".to_owned()],
        "and only what the new closure adds is started"
    );
}

#[test]
fn nothing_running_means_a_switch_starts_everything_and_stops_nothing() {
    let switched = moved(&[], &["sonarr".to_owned(), "radarr".to_owned()]);
    assert!(switched.stopped.is_empty() && switched.kept.is_empty());
    assert_eq!(
        switched.started,
        vec!["sonarr".to_owned(), "radarr".to_owned()]
    );
}

/// A container that exists and is not running has nothing to stop, so naming it
/// would produce a Compose command that did nothing and a report that lied.
#[test]
fn a_service_that_is_not_up_is_not_something_to_stop() {
    let running = vec![
        service("prowlarr", State::Stopped),
        service("bazarr", State::Absent),
        service("flaresolverr", State::Failed),
    ];
    let switched = moved(&running, &[]);
    assert!(
        switched.stopped.is_empty(),
        "{:?} is stopped, absent or already exited",
        switched.stopped
    );
}

/// Crash-looping is the case that most wants stopping — it is up, repeatedly.
#[test]
fn a_crash_looping_service_outside_the_new_shape_is_stopped() {
    let running = vec![service("sabnzbd", State::CrashLooping)];
    assert_eq!(moved(&running, &[]).stopped, vec!["sabnzbd".to_owned()]);
}

/// The operating system owns a native Jellyfin, so a switch neither starts it
/// nor claims to have.
#[test]
fn a_host_managed_service_is_neither_started_nor_stopped() {
    let running = vec![service("jellyfin", State::HostManaged)];
    let switched = moved(&running, &["jellyfin".to_owned()]);
    assert!(
        switched.started.is_empty() && switched.stopped.is_empty() && switched.kept.is_empty(),
        "{switched:?}"
    );
}

/// The subtle one: Compose will not accept a service name whose profile is not
/// active, so the stop command has to name the profiles being left.
#[test]
fn the_stop_names_the_profiles_it_is_leaving_not_the_ones_it_is_going_to() {
    let plan = Manifest::from_toml(STACK)
        .ok()
        .map(|manifest| leaving(&manifest, &["qbittorrent".to_owned()]));
    assert!(
        plan.as_ref()
            .is_some_and(|plan| plan.profiles.iter().any(|profile| profile == "torrent")),
        "{plan:?}"
    );
}
