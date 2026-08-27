//! The one address to hand somebody who lives here.
//!
//! [`crate::door`] decides which service that is from what the stack declares; this
//! asks the engine what became of it and assembles the answer every surface shows.
//! The reading of what is running is the same survey the status view is built from,
//! so the two cannot grade one service differently.

use super::Ctx;
use crate::door::{address, begins_at, facing, Address, Facing};
use crate::error::{Diagnose, Problem};
use crate::model::{Beside, FrontDoorReport, Standing};
use crate::platform::Environment;

/// What there is to hand somebody who lives here, and where it stands.
pub(super) async fn front_door(ctx: &Ctx) -> Result<FrontDoorReport, Box<Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|err| Box::new(err.problem()))?;
    let containers = ctx
        .engine
        .list(&ctx.settings.project)
        .await
        .map_err(|err| Box::new(err.problem()))?;
    let profiles: Vec<String> = manifest
        .profiles
        .iter()
        .map(|profile| profile.id.clone())
        .collect();
    let running = crate::docker::survey(&manifest, &profiles, &containers);
    // Asked now rather than remembered: a machine renamed since the last look
    // answers as it is, which is the whole of how a changed address is noticed.
    let named = ctx.site.name().await;
    Ok(assembled(
        &manifest.services,
        &running,
        named.as_deref(),
        ctx.settings.household_host.as_deref(),
        ctx.environment,
    ))
}

/// The answer itself, over what the stack declares and what became of it.
///
/// Apart from the reading above so that the stacks worth asking about — one that
/// publishes nothing to the household at all — can be put to it, which no stack this
/// repository carries is.
fn assembled(
    declared: &[lemonfiber_manifest::Service],
    running: &[crate::docker::Service],
    named: Option<&str>,
    recorded: Option<&str>,
    environment: Environment,
) -> FrontDoorReport {
    let Some((chosen, service)) = begins_at(declared) else {
        return FrontDoorReport {
            standing: Standing::Absent,
            service: None,
            address: None,
            facing: None,
            meaning: NOWHERE.to_owned(),
            beside: beside(declared, None),
        };
    };

    let answering = running
        .iter()
        .find(|running| running.id == service.id)
        .is_some_and(|running| answering(running.state));
    let standing = standing(chosen, answering);
    let reached = reached(service, named, recorded, environment);
    FrontDoorReport {
        standing,
        service: Some(service.name.clone()),
        facing: Some(chosen),
        meaning: meaning(standing, &service.name, reached.is_none()),
        address: reached,
        beside: beside(declared, Some(service.id.as_str())),
    }
}

/// Where the door is reached from another device in the house.
///
/// Nothing for a service the stack publishes no port for: an address with no port
/// on it is one a browser answers with a refusal, and there is nothing to guess
/// at — the manifest is where a port is declared.
fn reached(
    service: &lemonfiber_manifest::Service,
    named: Option<&str>,
    recorded: Option<&str>,
    environment: Environment,
) -> Option<Address> {
    address(named, recorded, environment, service.port?)
}

/// Whether a service in this state could answer somebody arriving at it.
///
/// A container that has not finished starting has not begun answering, which is
/// what makes it different from one that is up: a door reported as open the moment
/// its container exists is a door somebody is sent to before it opens.
const fn answering(state: crate::docker::State) -> bool {
    matches!(
        state,
        crate::docker::State::Healthy
            | crate::docker::State::Running
            | crate::docker::State::HostManaged
    )
}

/// What is said where this stack publishes nothing anybody could begin at.
const NOWHERE: &str = "There is no front door. Nothing this stack runs for the household is \
                       somewhere to begin, so there is no address to hand anybody — and the \
                       one thing worse than saying so would be handing over an address that \
                       leads somewhere they cannot use.";

/// Where the door stands, from what it is and whether it is answering.
const fn standing(chosen: Facing, answering: bool) -> Standing {
    if !answering {
        return Standing::Unreachable;
    }
    match chosen {
        Facing::Watching => Standing::LibraryOnly,
        _ => Standing::Established,
    }
}

/// What is said where there is a door and nothing here can work out its address.
///
/// The state a fresh install on a machine whose own name is not published is in:
/// the stack ships its household links pointed at this machine and nowhere else,
/// which is the right default for a machine nobody has told where it is and the
/// wrong address to hand anybody. So it is not handed over — it is said, with the
/// one thing that fixes it.
const UNADDRESSED: &str = " Nothing here can work out an address for this machine that another                            device would reach: it does not publish its own name, and the                            address the household's links point at is still the one that means                            this machine and nowhere else. Set `HOMEPAGE_VAR_LAN_HOST` to this                            machine's address on your network and it will be the address given                            here.";

/// What this comes to, in the words an operator would say it in.
fn meaning(standing: Standing, name: &str, unaddressed: bool) -> String {
    let said = said(standing, name);
    if unaddressed && standing != Standing::Absent {
        return format!("{said}{UNADDRESSED}");
    }
    said
}

/// What the standing itself comes to, before anything is said about the address.
fn said(standing: Standing, name: &str) -> String {
    match standing {
        Standing::Established => format!(
            "Send them to {name}. It is where they ask for what they want, and it links \
             onward to where they watch it."
        ),
        Standing::LibraryOnly => format!(
            "Send them to {name}. This stack has nowhere to ask for anything, so what is \
             already there is what there is to watch."
        ),
        Standing::Unreachable => format!(
            "{name} is the front door and it is not answering, so there is nowhere to send \
             anybody yet. Nothing else here is a stand-in for it."
        ),
        Standing::Absent => NOWHERE.to_owned(),
    }
}

/// Everything else the household can reach, and why none of it is the door.
///
/// In the order the manifest declares them, so the same stack answers the same way
/// twice. The door itself is left out: it is named above, and naming it here as well
/// would be one fact stated in two places that can disagree.
fn beside(services: &[lemonfiber_manifest::Service], door: Option<&str>) -> Vec<Beside> {
    services
        .iter()
        .filter(|service| Some(service.id.as_str()) != door)
        .filter_map(|service| {
            let facing = facing(service)?;
            Some(Beside {
                service: service.name.clone(),
                facing,
                because: facing.because().to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{answering, assembled, front_door, meaning, NOWHERE, UNADDRESSED};
    use crate::door::fixtures::{asking, service, watching};
    use crate::door::Facing;
    use crate::model::Standing;
    use crate::platform::Environment;
    use crate::ports::docker::{Health, Lifecycle};
    use crate::test_support::{a_context, Reporting};
    use lemonfiber_fixtures::ports::Renamed;

    /// The stack this repository carries, with the named services running and well.
    fn ctx(up: &[&str]) -> crate::app::Ctx {
        a_context()
            .engine(std::sync::Arc::new(Reporting::holding(
                up,
                Lifecycle::Running,
                Health::Healthy,
            )))
            .build()
    }

    #[tokio::test]
    async fn the_request_service_is_the_door_and_the_library_stands_beside_it() {
        let report = front_door(&ctx(&["seerr", "jellyfin"])).await;
        let report = report.ok();
        assert_eq!(
            report.as_ref().map(|report| report.standing),
            Some(Standing::Established)
        );
        assert_eq!(
            report.as_ref().and_then(|report| report.service.clone()),
            Some("Seerr".to_owned())
        );
        assert_eq!(
            report.as_ref().and_then(|report| report.facing),
            Some(Facing::Asking)
        );
        let beside: Vec<String> = report
            .iter()
            .flat_map(|report| report.beside.iter())
            .map(|beside| beside.service.clone())
            .collect();
        assert!(beside.contains(&"Jellyfin".to_owned()));
        assert!(!beside.contains(&"Seerr".to_owned()));
    }

    #[tokio::test]
    async fn the_index_over_everything_is_named_beside_the_door_and_refused() {
        let report = front_door(&ctx(&["seerr"])).await.ok();
        let homepage = report
            .iter()
            .flat_map(|report| report.beside.iter())
            .find(|beside| beside.service == "Homepage")
            .map(|beside| (beside.facing, beside.because.clone()));
        assert_eq!(
            homepage,
            Some((Facing::Operators, Facing::Operators.because().to_owned()))
        );
    }

    #[tokio::test]
    async fn a_door_nothing_is_running_behind_is_said_to_be_unreachable() {
        // Nothing is up, so the door this stack declares is not answering — which is
        // not the same answer as there being no door.
        let report = front_door(&ctx(&[])).await.ok();
        assert_eq!(
            report.as_ref().map(|report| report.standing),
            Some(Standing::Unreachable)
        );
        assert_eq!(
            report.and_then(|report| report.service),
            Some("Seerr".to_owned())
        );
    }

    #[tokio::test]
    async fn the_address_is_the_one_this_machine_answers_to_now() {
        let renamed = Renamed::called(Some("kitchen-nas")).then(Some("cupboard-nas"));
        let household = a_context()
            .engine(std::sync::Arc::new(Reporting::holding(
                &["seerr"],
                Lifecycle::Running,
                Health::Healthy,
            )))
            .environment(Environment::MacOs)
            .build()
            .with_site(std::sync::Arc::clone(&renamed) as std::sync::Arc<dyn crate::ports::Site>);

        let first = front_door(&household)
            .await
            .ok()
            .and_then(|report| report.address);
        assert_eq!(
            first.map(|address| address.url),
            Some("http://kitchen-nas.local:5055".to_owned())
        );

        // The machine has been renamed and says something else about itself. Nothing
        // was kept, so the second answer is the second answer rather than the first.
        let again = front_door(&household)
            .await
            .ok()
            .and_then(|report| report.address);
        assert_eq!(
            again.map(|address| address.url),
            Some("http://cupboard-nas.local:5055".to_owned())
        );
        assert_eq!(renamed.times(), 2);
    }

    #[tokio::test]
    async fn a_machine_nothing_can_address_is_told_what_to_set() {
        // A fresh install on a host that does not publish its own name: the stack's
        // household links still point at this machine and nowhere else, so there is
        // no address to hand anybody and saying so is the answer.
        let household = a_context()
            .engine(std::sync::Arc::new(Reporting::holding(
                &["seerr"],
                Lifecycle::Running,
                Health::Healthy,
            )))
            .environment(Environment::LinuxNative)
            .build()
            .with_site(Renamed::called(Some("kitchen-nas")));
        let report = front_door(&household).await.ok();
        assert_eq!(
            report.as_ref().and_then(|report| report.address.clone()),
            None
        );
        assert!(report.is_some_and(|report| report.meaning.contains("HOMEPAGE_VAR_LAN_HOST")));
    }

    #[tokio::test]
    async fn the_address_the_operator_recorded_is_the_one_that_is_given() {
        let recorded = crate::config::Settings {
            household_host: Some("192.168.1.10".to_owned()),
            ..crate::config::Settings::default()
        };
        let household = a_context()
            .engine(std::sync::Arc::new(Reporting::holding(
                &["seerr"],
                Lifecycle::Running,
                Health::Healthy,
            )))
            .environment(Environment::LinuxNative)
            .settings(recorded)
            .build()
            .with_site(Renamed::called(None));
        let address = front_door(&household)
            .await
            .ok()
            .and_then(|report| report.address);
        assert_eq!(
            address.as_ref().map(|address| address.url.clone()),
            Some("http://192.168.1.10:5055".to_owned())
        );
        assert!(address.and_then(|address| address.caution).is_some());
    }

    #[test]
    fn a_door_with_no_address_says_so_and_a_stack_with_no_door_does_not() {
        // The two absences are different: one has somewhere to send people and no
        // way to say where, and the other has nowhere to send them at all.
        assert!(meaning(Standing::Established, "Seerr", true).contains(UNADDRESSED.trim()));
        assert!(!meaning(Standing::Established, "Seerr", false).contains(UNADDRESSED.trim()));
        assert_eq!(meaning(Standing::Absent, "Seerr", true), NOWHERE);
    }

    #[tokio::test]
    async fn a_stack_that_cannot_be_read_is_a_problem_rather_than_no_door() {
        let ctx = a_context().over(crate::test_support::nowhere()).build();
        assert!(front_door(&ctx).await.is_err());
    }

    #[tokio::test]
    async fn an_engine_nothing_can_reach_is_a_problem_rather_than_a_door_that_is_down() {
        // Not knowing what is running is a different answer from knowing nothing is,
        // and reporting the second would be a guess dressed as a reading.
        let ctx = a_context().build();
        assert!(front_door(&ctx).await.is_err());
    }

    /// A running service, as the survey reports one.
    fn up(id: &str, state: crate::docker::State) -> crate::docker::Service {
        crate::docker::Service {
            id: id.to_owned(),
            name: id.to_owned(),
            profile: "media".to_owned(),
            state,
            criticality: lemonfiber_manifest::Criticality::Important,
            depends_on: Vec::new(),
            exit: None,
        }
    }

    #[test]
    fn a_stack_that_publishes_nothing_to_the_household_is_told_there_is_no_door() {
        // The operator-only configuration: everything it runs answers this machine
        // and nothing else, so there is no address to hand anybody at all.
        let declared = [service(
            "sonarr",
            Some(lemonfiber_manifest::Bind::Loopback),
            Some(lemonfiber_manifest::ApiKind::Servarr),
        )];
        let report = assembled(
            &declared,
            &[up("sonarr", crate::docker::State::Healthy)],
            Some("kitchen-nas"),
            None,
            Environment::MacOs,
        );
        assert_eq!(report.standing, Standing::Absent);
        assert_eq!(report.service, None);
        assert_eq!(report.facing, None);
        assert_eq!(report.meaning, NOWHERE);
        assert!(report.beside.is_empty());
    }

    #[test]
    fn a_stack_with_only_a_library_makes_the_library_the_door() {
        let declared = [watching()];
        let report = assembled(
            &declared,
            &[up("jellyfin", crate::docker::State::Healthy)],
            Some("kitchen-nas"),
            None,
            Environment::MacOs,
        );
        assert_eq!(report.standing, Standing::LibraryOnly);
        assert_eq!(report.service, Some("jellyfin".to_owned()));
        assert_eq!(report.facing, Some(Facing::Watching));
    }

    #[test]
    fn a_door_the_operating_system_runs_is_answering_like_any_other() {
        // A media server the host owns rather than Compose still answers arrivals,
        // so a stack running one is not reported as having a door that is down.
        let declared = [watching()];
        let report = assembled(
            &declared,
            &[up("jellyfin", crate::docker::State::HostManaged)],
            Some("kitchen-nas"),
            None,
            Environment::MacOs,
        );
        assert_eq!(report.standing, Standing::LibraryOnly);
    }

    #[test]
    fn a_door_still_starting_has_not_begun_answering() {
        assert!(!answering(crate::docker::State::Starting));
        assert!(!answering(crate::docker::State::Unhealthy));
        assert!(answering(crate::docker::State::Running));
        let declared = [asking()];
        let report = assembled(
            &declared,
            &[up("seerr", crate::docker::State::Starting)],
            Some("kitchen-nas"),
            None,
            Environment::MacOs,
        );
        assert_eq!(report.standing, Standing::Unreachable);
    }

    #[test]
    fn nothing_stands_in_for_a_door_that_is_not_answering() {
        let said = meaning(Standing::Unreachable, "Seerr", false);
        assert!(said.contains("Seerr"));
        assert!(said.contains("stand-in"));
    }

    #[test]
    fn a_library_only_stack_is_told_there_is_nowhere_to_ask() {
        let said = meaning(Standing::LibraryOnly, "Jellyfin", false);
        assert!(said.contains("nowhere to ask"));
    }

    #[test]
    fn no_door_at_all_says_so_rather_than_naming_the_nearest_thing() {
        assert_eq!(meaning(Standing::Absent, "Homepage", false), NOWHERE);
        assert!(!NOWHERE.contains("Homepage"));
    }
}
