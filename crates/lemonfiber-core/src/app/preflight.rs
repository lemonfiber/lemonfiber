//! What is already answering on the ports this stack is about to want.
//!
//! Two stacks on one host is a supported arrangement — distinct project names, and
//! ports that do not overlap — and the second half of that is the one nobody can
//! check by reading their own configuration. The comparison that finds an overlap
//! has existed for as long as the migration survey has, and the survey was its only
//! caller: an operator who never ran `migrate` met the same overlap as a Compose
//! error naming a port and nothing else, part-way through a start that had already
//! brought half the stack up.
//!
//! So it is asked before a start as well. Asked, not enforced: what is on this
//! machine is the operator's, a port they deliberately share is their business, and
//! refusing to start over it would make this the thing standing between them and
//! their stack. The start goes ahead and the report says what it found, which is the
//! difference between an operator who knows why the bind failed and one who does not.
//!
//! Two questions, because a port can be held by something that is not a container at
//! all: a media server the operator installed themselves, a service the system
//! started at boot, a development server somebody left running. The engine cannot see
//! any of them, so the ports it cannot account for are put to the machine itself.
//!
//! Nothing here refuses to answer either. An engine that will not say which projects
//! it holds leaves an empty list rather than a failure, and a machine that will not
//! say what is listening leaves the same, because a start that cannot be pre-flighted
//! is still a start the operator asked for.

use std::collections::BTreeSet;

use lemonfiber_manifest::Manifest;

use crate::migration::{pins, standing, Ours};
use crate::model::ConflictReport;
use crate::ports::docker::{Container, Image};

use super::Ctx;

/// Every host port the services in this plan would publish that another Compose
/// project on this machine already answers on.
///
/// Narrowed to the plan rather than to the whole manifest, because a conflict on a
/// service that is not being started is not a conflict with anything: an operator
/// running only the television form has no quarrel with whoever holds the music
/// service's port.
///
/// lemonfiber's own project is excluded by the comparison itself, so bringing this
/// stack up a second time reports nothing — it is the one project whose ports are
/// meant to be these.
///
/// Nothing is asked of the engine where the answer is already settled: a plan holding
/// no service that publishes a port has nothing to clash over, and that is the common
/// case for everything except a start.
pub(crate) async fn conflicting_ports(
    ctx: &Ctx,
    manifest: &Manifest,
    starting: &[String],
) -> Vec<ConflictReport> {
    // What this start would want, worked out before anything is asked of the engine.
    // A plan that publishes nothing cannot clash with anything, and a question to a
    // daemon whose answer is decided in advance is a question worth not asking — a
    // teardown, a stack of services that bind nothing, and a machine with no engine
    // reachable at all all come out here.
    let ours: Vec<Ours> = pins(manifest)
        .into_iter()
        .filter(|one| one.port.is_some())
        .filter(|one| starting.iter().any(|service| service == &one.service))
        .collect();
    if ours.is_empty() {
        return Vec::new();
    }

    let Ok(images) = ctx.images.images().await else {
        return Vec::new();
    };
    let Some(seen) = every_container(ctx, &images).await else {
        return Vec::new();
    };
    let standing = standing::here(&ctx.settings.project, &seen, &ours);
    let mut found = standing::conflicts(&ours, &standing);
    found.extend(outside_the_engine(ctx, &ours, &seen).await);
    found
}

/// What a port nothing in the container engine explains is said to be held by.
///
/// Deliberately vague about who, because that is as much as can be honestly said: the
/// programs asked report a listener, and the operator knows what they run.
const OUTSIDE: &str = "something on this machine outside your container engine";

/// Every port this plan wants that something not in a container is already holding.
///
/// The comparison above reads the container engine, so the whole of what it can see
/// is containers — and the ordinary way a start fails to bind is a program the
/// operator installed themselves, sitting on a port a service here also wants. To
/// that comparison such a port is free, and the first anybody hears of it is Compose
/// failing part-way through a start.
///
/// Only ports no container explains are asked about, and every container on this
/// machine counts — lemonfiber's own included. Our containers publish exactly these
/// ports while the stack is up, and a machine is perfectly able to see them
/// listening: without that exclusion, starting a stack that is already running would
/// report every one of its own ports as held by a stranger.
async fn outside_the_engine(ctx: &Ctx, ours: &[Ours], seen: &[Container]) -> Vec<ConflictReport> {
    let explained: BTreeSet<u16> = seen
        .iter()
        .flat_map(|container| container.published.iter())
        .map(|published| published.port)
        .collect();
    let unaccounted: Vec<u16> = ours
        .iter()
        .filter_map(|one| one.port)
        .filter(|port| !explained.contains(port))
        .collect();

    let held = ctx.site.answering_on(&unaccounted).await;
    let mut found: Vec<ConflictReport> = ours
        .iter()
        .filter_map(|one| one.port.map(|port| (one, port)))
        .filter(|(_, port)| held.contains(port))
        .map(|(one, port)| ConflictReport {
            port,
            wanted_by: one.service.clone(),
            held_by: OUTSIDE.to_owned(),
        })
        .collect();
    found.sort_by(|one, two| {
        one.port
            .cmp(&two.port)
            .then(one.wanted_by.cmp(&two.wanted_by))
    });
    found
}

/// Every container of every Compose project this engine knows, or nothing where it
/// would not say.
///
/// The project names come from the images the engine holds, because there is no ask
/// for "every project" — a project is a label on a container, and the images are what
/// carries the set of them without listing every container on the machine first.
///
/// `None` where any part of the walk failed, and the distinction is load-bearing for
/// the survey that shares this: that nothing was found and that nothing could be
/// looked at are different facts, and only one of them makes it safe to stand a stack
/// up here.
pub(crate) async fn every_container(ctx: &Ctx, images: &[Image]) -> Option<Vec<Container>> {
    let mut projects: BTreeSet<&str> = BTreeSet::new();
    for image in images {
        projects.extend(
            image
                .projects
                .iter()
                .map(String::as_str)
                .filter(|project| !project.is_empty()),
        );
    }

    let mut seen: Vec<Container> = Vec::new();
    for project in projects {
        seen.extend(ctx.engine.list(project).await.ok()?);
    }
    Some(seen)
}

#[cfg(test)]
mod tests {
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
}
