//! The two questions this product puts to the host it is running on.
//!
//! What this machine is called, which the address handed to the household is built
//! from; and what is already answering on the ports a start is about to want, which
//! is the difference between an operator who knows why a bind failed and one who
//! does not. Both go out through the same seam, and both are asked of the machine
//! rather than of anything lemonfiber configured — so on a real host the answers
//! differ from minute to minute and from laptop to laptop.
//!
//! Which is why they are driven from here rather than left to whichever test each
//! half belongs to. A stand-in for this seam has to answer both questions, and a
//! stand-in that has never been asked one of them is a stand-in whose answer to it
//! nobody has read. Each test below puts both questions to one machine and says what
//! that machine is: one that knows its own name and is holding nothing, and one
//! holding a port with no name to give.

use std::sync::Arc;

use common::stack::project;

use crate::common;
use lemonfiber_core::app::{dispatch, started, Command, Ctx, Outcome};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::model::{FrontDoorReport, LifecycleReport};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::files::Files;
use lemonfiber_fixtures::ports::{Bound, Renamed};
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_ports::network::Site;

/// A context over the stack this repository carries, asking the given seam what this
/// machine is.
///
/// The household's own services are reported up, because the door grades what it
/// finds running and a door nobody could arrive at answers with no address for a
/// reason that has nothing to do with the machine's name. The engine holds no images,
/// so no container explains any of the stack's ports and every one of them is put to
/// the machine itself — which is the path both questions have to travel for either
/// answer to mean anything.
fn over(site: Arc<dyn Site>) -> Ctx {
    lemonfiber_testing::a_context()
        .engine(Arc::new(Reporting::holding(
            &["seerr", "jellyfin"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .filesystem(Files::empty())
        .settings(Settings {
            protocols: Protocols::both(),
            ..Settings::default()
        })
        .build()
        .with_site(site)
        .with_images(Pulled::holding(Vec::new()))
}

/// The media server this repository's stack ships, and the host port it publishes.
///
/// Read out of the stack rather than written down here: a number spelled in this file
/// would stop being a conflict with anything the moment somebody moved it, and the
/// test would go on passing while proving nothing.
fn publisher() -> (String, u16) {
    let read = Source::External(project()).manifest();
    let Ok(manifest) = read else {
        unreachable!("the stack this repository carries is one its own parser reads")
    };
    let found = manifest
        .services
        .iter()
        .find(|service| service.id == "jellyfin")
        .and_then(|service| service.port.map(|port| (service.id.clone(), port)));
    let Some(pair) = found else {
        unreachable!("the stack declares a media server publishing a host port")
    };
    pair
}

/// What a start would run into, or the reason it could not be asked.
///
/// The status says the command failed, so nothing is waited on: what is under test is
/// settled before anything is spawned, and waiting would only add a minute to it.
async fn lifecycle(ctx: &Ctx) -> Result<LifecycleReport, String> {
    match started(ctx, &[], &[], Some(1)).await {
        Ok(report) => Ok(report),
        Err(problem) => Err(problem.summary.clone()),
    }
}

/// The answer the household is handed, or nothing where it could not be assembled.
async fn front_door(ctx: &Ctx) -> Option<FrontDoorReport> {
    match dispatch(Command::FrontDoor, ctx).await {
        Ok(Outcome::FrontDoor(report)) => Some(report),
        _ => None,
    }
}

/// The address that answer carries, as text, or nothing where it carries none.
fn addressed(report: Option<FrontDoorReport>) -> Option<String> {
    report
        .and_then(|report| report.address)
        .map(|address| address.url)
}

/// A machine that answers with its name is answering about itself, and it is holding
/// none of the ports the stack is about to bind.
///
/// The two go together rather than being two tests, because they are two questions to
/// one host and the pairing is the claim. A machine that says it is `kitchen-nas` and
/// then says it is already answering on the media server's port is describing a host
/// where this stack is half up — a real arrangement, and not this one. Reading the two
/// answers from one machine is what stops a start reporting every port of the stack it
/// is bringing up as held by a stranger.
#[tokio::test]
async fn a_machine_that_says_what_it_is_called_is_holding_none_of_the_stacks_ports() {
    let ctx = over(Renamed::called(Some("kitchen-nas")));

    // The door asks what this machine is called, because the one address handed to
    // the people in the house is built out of it.
    let url = addressed(front_door(&ctx).await).unwrap_or_default();
    assert!(url.contains("kitchen-nas"), "{url}");

    // The start asks the same machine what is already answering on the ports it is
    // about to want, and this one is answering on none of them.
    let clashes = lifecycle(&ctx)
        .await
        .map(|report| report.port_conflicts.len());
    assert_eq!(clashes, Ok(0));
}

/// A machine already answering on a port the start wants says which one, and has no
/// name to answer the other question with.
///
/// The gap this closes is a program the operator installed themselves: nothing in the
/// container engine explains it, so the comparison that reads the engine calls the
/// port free, and the first anybody hears of it is Compose failing part-way through a
/// start that has already brought half the stack up. The start still goes ahead — what
/// is on this machine is the operator's — and the report names the port and the
/// service that wanted it.
///
/// The other half is the honest absence. This machine will say what is bound on it and
/// not what it is called, and a door with nowhere to send anybody says so rather than
/// handing out an address built from a name nobody gave.
#[tokio::test]
async fn a_machine_holding_a_port_the_start_wants_says_so_and_has_no_name_to_give() {
    let (service, port) = publisher();
    let ctx = over(Bound::holding(&[port]));

    let held = lifecycle(&ctx).await.map(|report| {
        report
            .port_conflicts
            .iter()
            .map(|clash| (clash.port, clash.wanted_by.clone()))
            .collect::<Vec<(u16, String)>>()
    });
    assert_eq!(held, Ok(vec![(port, service)]));

    let url = addressed(front_door(&ctx).await);
    assert_eq!(url, None);
}
