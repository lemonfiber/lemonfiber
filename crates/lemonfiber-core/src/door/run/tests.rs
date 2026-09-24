use super::{answering, assembled, front_door, meaning, NOWHERE, UNADDRESSED};
use crate::door::fixtures::{asking, service, watching};
use crate::door::{Chosen, Facing, Refusal, KEPT};
use crate::model::Standing;
use crate::platform::Environment;
use crate::ports::docker::{Health, Lifecycle};
use crate::test_support::{a_context, Reporting};
use lemonfiber_fixtures::ports::Renamed;

/// The stack this repository carries, with the named services running and well,
/// on a machine that says what it is called.
///
/// The name is part of the fixture rather than left out, because a door with no
/// address is a *different* standing now — and a test that meant to be about
/// which service the door is would otherwise be about that instead. It was:
/// this fixture answered nothing, and the assertion beneath it said
/// `Established` for a door nobody could arrive at.
fn ctx(up: &[&str]) -> crate::app::Ctx {
    a_context()
        .engine(std::sync::Arc::new(Reporting::holding(
            up,
            Lifecycle::Running,
            Health::Healthy,
        )))
        .build()
        .with_site(Renamed::called(Some("kitchen-nas")))
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

/// The library is reachable even where the request service is the door.
///
/// The point of carrying an address beside the door rather than only on it: a
/// member who wants to watch something wants the service that faces watching,
/// and which of the two the operator made the front door is not an answer to
/// that question. Without this a surface can hand them only whichever one it
/// turned out to be.
#[tokio::test]
async fn the_library_beside_the_door_can_still_be_handed_over() {
    let report = front_door(&ctx(&["seerr", "jellyfin"])).await.ok();
    let library = report
        .iter()
        .flat_map(|report| report.beside.iter())
        .find(|beside| beside.service == "Jellyfin")
        .and_then(|beside| beside.address.clone());

    assert!(
        library.is_some_and(|address| address.url.contains("kitchen-nas")),
        "the library stood beside the door with nowhere to send anybody"
    );
}

/// The index over every service is named and is **not** handed over.
///
/// It says of itself that it includes the services nobody in the house should
/// learn exist. Naming it beside the door is how it gets refused by name; an
/// address on it would be that refusal undone, so the two halves of the answer
/// have to disagree about it on purpose.
#[tokio::test]
async fn the_index_over_everything_is_named_without_being_handed_over() {
    let report = front_door(&ctx(&["seerr"])).await.ok();
    let homepage = report
        .iter()
        .flat_map(|report| report.beside.iter())
        .find(|beside| beside.service == "Homepage");

    assert_eq!(
        homepage.map(|beside| beside.facing),
        Some(Facing::Operators),
        "the index is not somewhere the household begins"
    );
    assert!(
        homepage.is_some_and(|beside| beside.address.is_none()),
        "the operator's index was handed to the household"
    );
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
    assert!(meaning(Standing::Stranded, "Seerr", &Chosen::Derived).contains(UNADDRESSED.trim()));
    assert!(!meaning(Standing::Established, "Seerr", &Chosen::Derived).contains(UNADDRESSED.trim()));
    assert_eq!(
        meaning(Standing::Absent, "Seerr", &Chosen::Derived),
        NOWHERE
    );
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
        describes: format!("what {id} is for"),
        profile: "media".to_owned(),
        forms: Vec::new(),
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
        None,
        Environment::MacOs,
    );
    assert_eq!(report.standing, Standing::Unreachable);
}

/// The two ways a door is unreachable are two answers, not one with a caveat.
///
/// One is fixed by starting a service and the other by giving this machine an
/// address, so a household member who cannot arrive is told which end the
/// problem is at before they start blaming their own device. Asserted on the
/// standing rather than on the sentence: the sentence already distinguished
/// them and the field every other surface reads did not, which is the whole of
/// what was wrong.
#[tokio::test]
async fn a_service_that_is_down_and_a_machine_with_no_address_are_two_answers() {
    let down = a_context()
        .engine(std::sync::Arc::new(Reporting::holding(
            &["jellyfin"],
            Lifecycle::Exited,
            Health::None,
        )))
        .build()
        .with_site(Renamed::called(Some("kitchen-nas")));
    let stranded = a_context()
        .engine(std::sync::Arc::new(Reporting::holding(
            &["seerr"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .environment(Environment::LinuxNative)
        .build()
        .with_site(Renamed::called(None));

    let down = front_door(&down).await.ok().map(|report| report.standing);
    let stranded = front_door(&stranded)
        .await
        .ok()
        .map(|report| report.standing);

    assert_eq!(down, Some(Standing::Unreachable), "the service is not up");
    assert_eq!(
        stranded,
        Some(Standing::Stranded),
        "the service is up and there is no way to it"
    );
    assert_ne!(down, stranded, "and they are not the same answer");
}

/// A door nobody can arrive at is not established.
///
/// The feature defines `established` as running **and reachable**. This
/// reported it for a door answering on a machine with no address, and said the
/// rest in prose — so a browser, a script or a dashboard reading the state was
/// told the door was fine while the sentence beneath it said otherwise.
#[tokio::test]
async fn a_door_with_no_address_is_not_reported_as_established() {
    let household = a_context()
        .engine(std::sync::Arc::new(Reporting::holding(
            &["seerr"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .environment(Environment::LinuxNative)
        .build()
        .with_site(Renamed::called(None));
    let report = front_door(&household).await.ok();

    assert_eq!(
        report.as_ref().map(|report| report.standing),
        Some(Standing::Stranded)
    );
    assert_eq!(report.as_ref().and_then(|r| r.address.clone()), None);
    // And what is said still carries the one thing that fixes it.
    assert!(report
        .as_ref()
        .is_some_and(|report| report.meaning.contains("HOMEPAGE_VAR_LAN_HOST")));
}

#[test]
fn nothing_stands_in_for_a_door_that_is_not_answering() {
    let said = meaning(Standing::Unreachable, "Seerr", &Chosen::Derived);
    assert!(said.contains("Seerr"));
    assert!(said.contains("stand-in"));
}

#[test]
fn a_library_only_stack_is_told_there_is_nowhere_to_ask() {
    let said = meaning(Standing::LibraryOnly, "Jellyfin", &Chosen::Derived);
    assert!(said.contains("nowhere to ask"));
}

#[tokio::test]
async fn the_door_the_operator_named_is_the_one_that_is_given() {
    // Their stack, their call. This one has somewhere to ask and its operator
    // would rather the household simply landed in the library.
    let chose = crate::config::Settings {
        front_door: Some("jellyfin".to_owned()),
        ..crate::config::Settings::default()
    };
    let household = a_context()
        .engine(std::sync::Arc::new(Reporting::holding(
            &["seerr", "jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(chose)
        .build()
        .with_site(Renamed::called(Some("kitchen-nas")));
    let report = front_door(&household).await.ok();

    assert_eq!(
        report.as_ref().and_then(|report| report.service.clone()),
        Some("Jellyfin".to_owned())
    );
    assert_eq!(
        report.as_ref().map(|report| report.chosen.clone()),
        Some(Chosen::Named("jellyfin".to_owned()))
    );
    assert_eq!(
        report.as_ref().map(|report| report.standing),
        Some(Standing::LibraryOnly)
    );
    // And what it costs is said in the answer, not only beside the setting.
    assert!(report.is_some_and(|report| report.meaning.contains(KEPT)));
}

#[tokio::test]
async fn a_named_door_the_household_tier_does_not_publish_is_refused_and_said() {
    // The refusal the setting is bounded by. Obeying it would publish an address
    // for a service that answers this machine alone — and a household member who
    // arrived there could change what everybody else gets.
    let chose = crate::config::Settings {
        front_door: Some("sonarr".to_owned()),
        ..crate::config::Settings::default()
    };
    let household = a_context()
        .engine(std::sync::Arc::new(Reporting::holding(
            &["seerr"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .settings(chose)
        .build()
        .with_site(Renamed::called(Some("kitchen-nas")));
    let report = front_door(&household).await.ok();

    let refused = report.as_ref().map(|report| report.chosen.clone());
    assert!(
        matches!(refused, Some(Chosen::Refused(Refusal { ref named, .. })) if named == "sonarr"),
        "{refused:?}"
    );
    assert_eq!(
        report.as_ref().and_then(|report| report.service.clone()),
        Some("Seerr".to_owned()),
        "the door this stack's own shape settles on still stands"
    );
    let said = report.map(|report| report.meaning).unwrap_or_default();
    assert!(said.contains("`sonarr`"), "{said}");
    assert!(said.contains("cannot be one"), "{said}");
}

#[test]
fn a_door_nobody_named_says_nothing_about_having_been_named() {
    // The sentence is the cost of a decision, and a stack whose operator made no
    // decision has not paid it.
    let derived = meaning(Standing::Established, "Seerr", &Chosen::Derived);
    assert!(!derived.contains(KEPT));
    assert!(!derived.contains("cannot be one"));
}

#[test]
fn a_refusal_is_said_even_where_there_was_no_door_to_fall_back_to() {
    // Two absences at once: nothing to send anybody to, and a setting that named
    // something that could not have been it either.
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
        Some("sonarr"),
        Environment::MacOs,
    );
    assert_eq!(report.standing, Standing::Absent);
    assert!(report.meaning.starts_with(NOWHERE));
    assert!(report.meaning.contains("`sonarr`"), "{}", report.meaning);
}

#[test]
fn no_door_at_all_says_so_rather_than_naming_the_nearest_thing() {
    assert_eq!(
        meaning(Standing::Absent, "Homepage", &Chosen::Derived),
        NOWHERE
    );
    assert!(!NOWHERE.contains("Homepage"));
}
