//! A service an operator added to their own stack, operated by a build that has never
//! heard of it.
//!
//! Driven against a fork rather than against the stack this repository ships, because
//! that is the only place the question arises: everything in the shipped stack is by
//! definition something the stack describes. The fixture beside this declares one
//! service lemonfiber ships and one it does not — no API declaration, a name nothing
//! here mentions, and, unlike the service beside it, nothing said about where it
//! reaches.
//!
//! Two halves, and they pull in opposite directions. The generic half must work: the
//! unknown service gets a status row, a state, a criticality, and a place in the
//! invocation that starts the form it belongs to. The specific half must *say so*:
//! anything needing to know what a service is answers about it rather than leaving it
//! out, because a report that is complete-looking and short is worse than one that
//! admits what it does not know.
//!
//! Nothing here runs a container. The invocation is read rather than spawned, which is
//! what lets a fork's service be proven operable with no daemon anywhere.

use std::sync::Arc;

use lemonfiber_core::app::{dispatch, Command, Ctx, Outcome};
use lemonfiber_core::config::{Protocols, Settings};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::support::Reporting;

/// The service in the fixture that this build has never heard of.
const THEIRS: &str = "pantograph";

/// The service in the fixture that it has.
const OURS: &str = "sonarr";

/// The stack the operator maintains, as an absolute path fixed at compile time.
fn fork() -> Source {
    Source::External(std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/somebody-elses-stack"
    )))
}

/// A context over that fork, whose engine reports the named services running.
fn ctx(up: &[&str]) -> Ctx {
    lemonfiber_testing::a_context()
        .runner(Arc::new(lemonfiber_fixtures::ports::Idle))
        .engine(Arc::new(Reporting::holding(
            up,
            Lifecycle::Running,
            Health::Healthy,
        )))
        .filesystem(lemonfiber_fixtures::files::Files::empty())
        // Named rather than left live: a start asks what else on this machine holds
        // the ports it wants, and a test that let that question reach a real daemon
        // would answer differently on every machine it ran on.
        .images(lemonfiber_fixtures::pulled::Pulled::holding(Vec::new()))
        .over(fork())
        .settings(Settings {
            protocols: Protocols::both(),
            ..Settings::default()
        })
        .build()
        .rehearsing()
}

/// Every service a status reading reports, by id.
async fn reported(up: &[&str]) -> Vec<lemonfiber_core::docker::Service> {
    match dispatch(Command::Ps { forms: Vec::new() }, &ctx(up)).await {
        Ok(Outcome::Status(report)) => report.services,
        _ => Vec::new(),
    }
}

/// A service lemonfiber has never heard of is reported like any other.
#[tokio::test]
async fn an_unknown_service_that_is_running_is_reported_as_running() {
    let services = reported(&[OURS, THEIRS]).await;
    let theirs = services.iter().find(|service| service.id == THEIRS);

    assert!(
        theirs.is_some(),
        "the operator's own service is missing from the reading: {services:?}"
    );
    assert!(
        theirs.is_some_and(|service| service.state == lemonfiber_core::docker::State::Healthy),
        "{services:?}"
    );
    // From the manifest rather than from anything this build knows about the name,
    // which is the whole of why it works at all.
    assert!(
        theirs.is_some_and(|service| service.name == "Pantograph"),
        "{services:?}"
    );
    assert!(
        theirs.is_some_and(
            |service| service.criticality == lemonfiber_manifest::Criticality::Enhancing
        ),
        "{services:?}"
    );
}

/// And one that has never started is reported absent rather than omitted, which is
/// the difference between reading the manifest and reading the container listing.
#[tokio::test]
async fn an_unknown_service_that_never_started_is_reported_absent_rather_than_left_out() {
    let services = reported(&[OURS]).await;
    let theirs = services.iter().find(|service| service.id == THEIRS);

    assert!(
        theirs.is_some_and(|service| service.state == lemonfiber_core::docker::State::Absent),
        "a service that never started is missing from the reading: {services:?}"
    );
}

/// Lifecycle control is by profile, so an unknown service in a named form is started
/// by the same invocation everything else in it is.
#[tokio::test]
async fn an_unknown_service_is_in_the_closure_and_in_the_command_that_starts_it() {
    let started = dispatch(
        Command::Up {
            forms: vec!["tv".to_owned()],
        },
        &ctx(&[]),
    )
    .await;

    let Ok(Outcome::Lifecycle(report)) = started else {
        unreachable!("a rehearsed start over a readable fork reports what it would run")
    };
    assert!(
        report.plan.services.iter().any(|service| service == THEIRS),
        "the closure left the operator's own service out: {:?}",
        report.plan.services
    );
    assert!(
        report.command.iter().any(|word| word == "--profile"),
        "the invocation names no profile: {:?}",
        report.command
    );
    // Nothing in the command names a service: a form starts a profile, and that is
    // what makes a service this build has never heard of start with the rest of it.
    assert!(
        !report.command.iter().any(|word| word == THEIRS),
        "the invocation names services one at a time, so an unknown one would need \
         lemonfiber to know it: {:?}",
        report.command
    );
}

/// The half that has to say so rather than work silently: an inventory of what leaves
/// this machine lists every service the stack declares, and admits which of them it
/// knows nothing about.
#[tokio::test]
async fn what_leaves_this_machine_lists_the_unknown_service_and_says_it_is_unknown() {
    let answered = dispatch(Command::Outbound, &ctx(&[])).await;

    let Ok(Outcome::Outbound(leaving)) = answered else {
        unreachable!("a readable fork is enumerated rather than refused")
    };
    let theirs = leaving.theirs.iter().find(|one| one.service == THEIRS);

    assert!(
        theirs.is_some(),
        "the operator's own service is not in the inventory at all: {:?}",
        leaving.theirs
    );
    assert!(
        theirs.is_some_and(|one| !one.recorded),
        "it is listed as though lemonfiber knew what it reaches: {theirs:?}"
    );
    // The one wrong answer: an empty destination is what a service that reaches
    // nothing says, and giving that to a service nobody has looked at would be a
    // privacy claim made out of ignorance.
    assert!(
        theirs.is_some_and(|one| !one.destination.is_empty()),
        "{theirs:?}"
    );
    // And the service beside it, which this operator's own stack does describe, is
    // answered about — from their manifest, which is the only place the answer comes
    // from and the reason a fork can correct it.
    assert!(
        leaving
            .theirs
            .iter()
            .any(|one| one.service == OURS && one.recorded),
        "{:?}",
        leaving.theirs
    );
}
