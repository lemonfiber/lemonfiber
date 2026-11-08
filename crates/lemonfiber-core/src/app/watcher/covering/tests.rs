use std::sync::Arc;

use lemonfiber_manifest::{Criticality, Health, HealthKind, Service};

use super::{beating, critical, entered, wanted, watched, Job, EXPIRING, GUARD, JOBS};
use crate::door::{Candidate, Facing, Place, Reached};
use crate::model::WatchPath;
use crate::ports::hosting::{Manager, Standing};
use crate::ports::service::{Covers, MonitorKind};
use crate::wiring::Fillers;
use lemonfiber_fixtures::hosting::Hosting;

/// The stack this build carries.
fn carried() -> Vec<Service> {
    crate::test_support::stack()
        .manifest()
        .map(|manifest| manifest.services)
        .unwrap_or_default()
}

/// The carried stack's fillers, written at `project`.
fn fillers(project: &std::path::Path) -> Fillers {
    crate::test_support::stack_fillers(
        carried(),
        &[],
        Some(project),
        crate::plugin::first_party::EMBEDDED,
    )
}

/// The carried service `id`, health and all, as the one service of a list, which is empty
/// where the stack carries none.
fn service(id: &str) -> Vec<Service> {
    carried().into_iter().filter(|one| one.id == id).collect()
}

#[test]
fn a_critical_service_is_one_its_form_or_the_world_outside_needs() {
    let mut sonarr = service("sonarr");
    assert_eq!(sonarr.len(), 1);
    for one in &mut sonarr {
        a_critical(one);
    }
}

/// `one` judged critical under each criticality.
fn a_critical(one: &mut Service) {
    for (criticality, is) in [
        (Criticality::Critical, true),
        (Criticality::Core, true),
        (Criticality::Important, false),
        (Criticality::Enhancing, false),
        (Criticality::Optional, false),
    ] {
        one.criticality = criticality;
        assert_eq!(critical(one), is, "{criticality:?}");
    }
}

#[test]
fn a_service_is_watched_at_its_health_path_its_port_or_its_container() {
    let mut sonarr = service("sonarr");
    assert_eq!(sonarr.len(), 1);
    for one in &mut sonarr {
        watched_every_way(one);
    }
}

/// `sonarr` watched under each health it could declare, and with no address at all.
fn watched_every_way(sonarr: &mut Service) {
    let project = lemonfiber_fixtures::scratch::Scratch::new("covering-services");
    let fillers = fillers(&project);
    let address = fillers
        .service("sonarr")
        .and_then(|filler| filler.address.clone())
        .map(|address| (address.url(), format!("{}:{}", address.host, address.port)))
        .unwrap_or_default();

    let http = watched("stack", sonarr, &fillers);
    assert_eq!(http.covers, Covers::Service("sonarr".to_owned()));
    assert_eq!(http.provision.kind, MonitorKind::Http);
    assert_eq!(http.provision.target, format!("{}/ping", address.0));
    assert_eq!(http.provision.tag, "lemonfiber:service:sonarr");
    assert_eq!((http.provision.every_s, http.provision.retries), (60, 3));

    sonarr.health = Some(Health {
        kind: HealthKind::Http,
        path: None,
        timeout_s: None,
    });
    assert_eq!(
        watched("stack", sonarr, &fillers).provision.target,
        format!("{}/", address.0)
    );

    for health in [
        None,
        Some(Health {
            kind: HealthKind::Tcp,
            path: None,
            timeout_s: None,
        }),
    ] {
        sonarr.health = health;
        let port = watched("stack", sonarr, &fillers).provision;
        assert_eq!(
            (port.kind, port.target),
            (MonitorKind::Port, address.1.clone())
        );
    }

    sonarr.health = Some(Health {
        kind: HealthKind::Container,
        path: None,
        timeout_s: None,
    });
    let docker = watched("stack", sonarr, &fillers).provision;
    assert_eq!(
        (docker.kind, docker.target),
        (MonitorKind::Docker, "stack-sonarr-1".to_owned())
    );

    sonarr.id = "nowhere-at-all".to_owned();
    let unaddressed = watched("stack", sonarr, &fillers).provision;
    assert_eq!(
        (unaddressed.kind, unaddressed.target),
        (MonitorKind::Docker, "stack-nowhere-at-all-1".to_owned())
    );
}

#[test]
fn an_entry_point_is_watched_over_the_households_address_and_said_where_it_cannot_be() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("covering-entries");
    let fillers = fillers(&project);
    let candidate = |id| Candidate {
        id,
        name: id,
        facing: Some(Facing::Asking),
        reached: Reached::Port(Some(5055)),
    };
    let known = Place {
        named: None,
        recorded: Some("192.168.1.20"),
        domain: None,
        environment: crate::platform::Environment::LinuxNative,
    };
    let household = entered(&candidate("seerr"), &known, &fillers);
    assert!(
        household
            .as_ref()
            .is_some_and(|one| one.provision.target == "http://192.168.1.20:5055"
                && one.path == Some(WatchPath::Household)
                && one.short.is_none()
                && one.covers == Covers::EntryPoint("seerr".to_owned())),
        "{household:?}"
    );

    let unknown = Place {
        recorded: None,
        ..known
    };
    let internal = entered(&candidate("seerr"), &unknown, &fillers);
    let address = fillers
        .service("seerr")
        .and_then(|filler| filler.address.as_ref())
        .map(crate::wiring::Address::url)
        .unwrap_or_default();
    assert!(
        internal
            .as_ref()
            .is_some_and(|one| one.provision.target == address
                && one.path == Some(WatchPath::Internal)
                && one
                    .short
                    .as_ref()
                    .is_some_and(|short| short.watches == address
                        && short.wanted == "the address the household is handed"
                        && short.why.starts_with("Nothing here can work out"))),
        "{internal:?}"
    );

    assert_eq!(
        entered(&candidate("nowhere-at-all"), &unknown, &fillers),
        None
    );
}

#[test]
fn a_job_is_watched_by_a_heartbeat_that_alarms_once_its_grace_is_spent() {
    let beat = beating(GUARD).provision;
    assert_eq!(beat.tag, "lemonfiber:job:guard");
    assert_eq!(beat.kind, MonitorKind::Heartbeat);
    assert_eq!(
        (beat.every_s, beat.grace_s, beat.retries),
        (60, Some(60), 0)
    );
    assert_eq!(beating(EXPIRING).provision.every_s, 3_600);
    assert_eq!(
        JOBS.map(Job::covers),
        [
            Covers::Job("guard".to_owned()),
            Covers::Job("expiring".to_owned())
        ]
    );
}

#[tokio::test]
async fn what_is_wanted_reads_the_households_address_as_recorded_and_the_jobs_hosted_now() {
    let project = lemonfiber_fixtures::scratch::Scratch::new("covering-wanted");
    let env = project.join(".env");
    let _ = std::fs::write(&env, "");
    let ctx = crate::test_support::a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            ..crate::config::Settings::default()
        })
        .build()
        .with_hosting(Hosting::holding(
            Manager::Launchd,
            "expiring",
            Hosting::installed("expiring", Standing::Stopped),
        ) as Arc<dyn crate::ports::Host>);
    let _ = crate::settings::store::recorded(
        &env.with_file_name(crate::config::paths::SETTINGS),
        &[(crate::config::HOUSEHOLD_HOST_SETTING, Some("192.168.1.20"))].into(),
    );

    let wanted = wanted(&ctx, &carried(), &[], &fillers(&project)).await;

    assert!(wanted
        .iter()
        .any(|one| one.path == Some(WatchPath::Household)
            && one.provision.target.starts_with("http://192.168.1.20:")));
    let jobs: Vec<&Covers> = wanted
        .iter()
        .map(|one| &one.covers)
        .filter(|covers| matches!(covers, Covers::Job(_)))
        .collect();
    assert_eq!(jobs, [&Covers::Job("expiring".to_owned())]);
}
