use super::{conflicting_ports, every_container, OUTSIDE};
use crate::app::Ctx;
use crate::ports::docker::{Health, Image, Lifecycle};
use crate::test_support::a_context;
use lemonfiber_fixtures::ports::Bound;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::Reporting;
use std::sync::Arc;

/// The manifest this build ships, which is what the plan's services are drawn
/// from — a port invented here would be a conflict with nothing.
///
/// An empty stack where this build cannot parse the one it embeds, which no
/// assertion below is satisfied by: a test that needed a service fails rather than
/// skipping quietly. Written out rather than reached through a way out of its own,
/// because a way out on a line of its own is a line no run enters — and the
/// coverage gate counts that against every honest line beside it.
fn manifest() -> lemonfiber_manifest::Manifest {
    crate::test_support::stack()
        .manifest()
        .unwrap_or(lemonfiber_manifest::Manifest {
            schema_version: 0,
            stack_version: String::new(),
            min_cli_version: String::new(),
            profiles: Vec::new(),
            forms: Vec::new(),
            services: Vec::new(),
            removed: Vec::new(),
            wirings: Vec::new(),
        })
}

/// The first service the shipped stack publishes a port for, and that port.
///
/// A nameless service on port zero where the stack publishes none, which no
/// assertion below is satisfied by: a build whose stack stopped publishing
/// anything fails the test that needed a port rather than skipping it quietly.
fn publisher() -> (String, u16) {
    publishers().first().cloned().unwrap_or_default()
}

/// Every service the shipped stack publishes a host port for, in the order the
/// stack declares them.
fn publishers() -> Vec<(String, u16)> {
    manifest()
        .services
        .iter()
        .filter_map(|service| service.port.map(|port| (service.id.clone(), port)))
        .collect()
}

/// A context whose engine holds one container of somebody else's project,
/// answering on `port`, and whose images say that project exists.
fn beside(project: &str, port: u16) -> Ctx {
    let engine = Reporting::holding(&["theirs"], Lifecycle::Running, Health::None)
        .belonging_to(project)
        .publishing(&[("theirs", "127.0.0.1", port)]);
    a_context()
        .engine(Arc::new(engine))
        .build()
        .with_images(Pulled::holding(vec![Pulled::image(
            "theirs:1",
            1,
            &[project],
        )]))
}

#[tokio::test]
async fn a_port_another_project_answers_on_is_named_on_both_sides() {
    let (service, port) = publisher();
    let ctx = beside("somebody-elses", port);

    let found = conflicting_ports(&ctx, &manifest(), std::slice::from_ref(&service)).await;

    assert_eq!(found.len(), 1, "{found:?}");
    let clash = found.first();
    assert!(clash.is_some_and(|one| one.port == port), "{found:?}");
    assert!(
        clash.is_some_and(|one| one.wanted_by == service),
        "{found:?}"
    );
    assert!(
        clash.is_some_and(|one| one.held_by.starts_with("somebody-elses/")),
        "{found:?}"
    );
}

/// A service the operator is not starting has no quarrel with anybody.
#[tokio::test]
async fn a_port_held_against_a_service_this_start_leaves_out_is_not_a_conflict() {
    let (service, port) = publisher();
    let ctx = beside("somebody-elses", port);
    let others: Vec<String> = manifest()
        .services
        .iter()
        .map(|one| one.id.clone())
        .filter(|id| id != &service)
        .collect();

    assert!(conflicting_ports(&ctx, &manifest(), &others)
        .await
        .is_empty());
}

/// Our own project is what these ports are meant to be, so a second start of this
/// stack reports nothing.
#[tokio::test]
async fn a_port_this_stack_already_holds_is_not_a_conflict_with_itself() {
    let (service, port) = publisher();
    let ctx = beside(crate::PRODUCT, port);

    assert!(conflicting_ports(&ctx, &manifest(), &[service])
        .await
        .is_empty());
}

/// An engine that will not say what it holds leaves the start unchecked rather
/// than refused: a pre-flight that could not look is not a reason to stop.
#[tokio::test]
async fn an_engine_that_will_not_list_its_images_reports_nothing() {
    let (service, _) = publisher();
    let ctx = a_context()
        .build()
        .with_images(Pulled::unreachable("no daemon here"));

    assert!(conflicting_ports(&ctx, &manifest(), &[service])
        .await
        .is_empty());
}

/// A plan wanting no port is settled without asking the engine anything.
#[tokio::test]
async fn a_plan_that_publishes_nothing_is_answered_without_asking_the_engine() {
    let (_, port) = publisher();
    let unpublished: Vec<String> = manifest()
        .services
        .iter()
        .filter(|service| service.port.is_none())
        .map(|service| service.id.clone())
        .collect();
    assert!(
        !unpublished.is_empty(),
        "the stack declares a service with no listener"
    );

    // An engine that would answer with a clash, and a plan that gives it nothing
    // to clash with: the answer is empty because of the plan, not the engine.
    let ctx = beside("somebody-elses", port);
    assert!(conflicting_ports(&ctx, &manifest(), &unpublished)
        .await
        .is_empty());
}

/// The gap this closes: a program the operator installed themselves is invisible
/// to a comparison that reads the container engine, and it binds first.
#[tokio::test]
async fn a_port_held_by_something_the_engine_cannot_see_is_named() {
    let (service, port) = publisher();
    let ctx = a_context()
        .build()
        .with_images(Pulled::holding(Vec::new()))
        .with_site(Bound::holding(&[port]));

    let found = conflicting_ports(&ctx, &manifest(), std::slice::from_ref(&service)).await;

    assert_eq!(found.len(), 1, "{found:?}");
    let clash = found.first();
    assert!(clash.is_some_and(|one| one.port == port), "{found:?}");
    assert!(
        clash.is_some_and(|one| one.wanted_by == service),
        "{found:?}"
    );
    assert!(clash.is_some_and(|one| one.held_by == OUTSIDE), "{found:?}");
}

/// Our own containers publish exactly these ports while the stack is up, and this
/// machine can see them listening. Starting a stack that is already running must
/// not report every one of its own ports as held by a stranger.
#[tokio::test]
async fn a_port_our_own_containers_publish_is_not_taken_for_a_stranger() {
    let (service, port) = publisher();
    let ctx = beside(crate::PRODUCT, port).with_site(Bound::holding(&[port]));

    assert!(conflicting_ports(&ctx, &manifest(), &[service])
        .await
        .is_empty());
}

/// A port another project holds is visible to both questions, and the operator is
/// owed the answer that names who has it rather than that one and a vaguer copy.
#[tokio::test]
async fn a_port_another_project_holds_is_not_reported_twice() {
    let (service, port) = publisher();
    let ctx = beside("somebody-elses", port).with_site(Bound::holding(&[port]));

    let found = conflicting_ports(&ctx, &manifest(), &[service]).await;

    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found
            .first()
            .is_some_and(|one| one.held_by.starts_with("somebody-elses/")),
        "{found:?}"
    );
}

/// A daemon that names a project and then will not say what is in it leaves the
/// start unchecked, rather than reporting that the ports are clear.
///
/// The walk keeps two answers apart on purpose: nothing was found, and nothing
/// could be looked at. Only the first is evidence that a port is free. An engine
/// that lists its images and then refuses the containers behind one of them has
/// answered neither, and a pre-flight that read that as an empty machine would
/// tell the operator their ports are clear on exactly the host it could not read
/// — which is worse than saying nothing, because they would believe it.
///
/// Reached past the image read deliberately: the earlier refusal is already its
/// own test, and this is the half that only shows up once there is a project
/// worth asking about.
#[tokio::test]
async fn an_engine_that_will_not_say_what_a_project_holds_reports_nothing() {
    let (service, _) = publisher();
    // Images enough to name one project, and an engine that answers nothing when
    // asked what that project holds — so the walk fails after it has something to
    // walk rather than before.
    let ctx = a_context()
        .engine(Arc::new(Reporting::absent()))
        .build()
        .with_images(Pulled::holding(vec![Pulled::image(
            "theirs:1",
            1,
            &["somebody-elses"],
        )]));

    assert!(conflicting_ports(&ctx, &manifest(), &[service])
        .await
        .is_empty());
}

/// Two ports a stranger holds are reported lowest first, whatever order the stack
/// declares the services that want them in.
///
/// What an operator does with this list is compare it with the last one. A report
/// ordered by however the manifest happens to be written would move when somebody
/// reorders a file that has nothing to do with them, and a list that moves for no
/// reason is a list people stop reading. The ordering is by port, and by the
/// service that wanted it where two want the same one, so the same machine answers
/// the same way twice.
#[tokio::test]
async fn two_ports_held_outside_the_engine_are_reported_lowest_first() {
    let declared = publishers();
    let (first, high) = declared.first().cloned().unwrap_or_default();
    let (second, low) = declared.get(1).cloned().unwrap_or_default();
    // Both nameless on port zero where the stack publishes fewer than two, which
    // the ordering assertion below refuses: a pair that does not exist cannot be
    // out of order, and a test that read one would pass having read nothing.
    assert!(
        high > low,
        "this reads the first two services the stack publishes ports for, and the \
         sort is only observable because the stack declares them in the opposite \
         order to their ports. It no longer does — pick two that are."
    );

    // No images, so nothing in the engine explains either port, and a machine
    // holding both: the pair comes back through the question put to the host.
    let ctx = a_context()
        .build()
        .with_images(Pulled::holding(Vec::new()))
        .with_site(Bound::holding(&[high, low]));
    let starting = [first.clone(), second.clone()];

    let found = conflicting_ports(&ctx, &manifest(), &starting).await;

    let ports: Vec<u16> = found.iter().map(|clash| clash.port).collect();
    assert_eq!(ports, vec![low, high], "{found:?}");
    let wanted: Vec<&str> = found.iter().map(|clash| clash.wanted_by.as_str()).collect();
    assert_eq!(wanted, vec![second.as_str(), first.as_str()], "{found:?}");
}

#[tokio::test]
async fn an_image_belonging_to_no_project_names_no_project_to_ask_about() {
    let ctx = a_context().build();
    let unlabelled = Image {
        tags: vec!["nobody:1".to_owned()],
        bytes: 1,
        projects: vec![String::new()],
    };

    assert_eq!(
        every_container(&ctx, std::slice::from_ref(&unlabelled)).await,
        Some(Vec::new())
    );
}
