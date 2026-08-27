//! What the stack is actually listening on, held against what it is meant to.
//!
//! Driven through the container-engine port, because the whole point of the check is
//! that it reads the runtime's own account rather than the files that asked for it —
//! and the way to prove that is to make the two disagree.

use std::sync::Arc;

use lemonfiber_core::doctor::bindings::BindingsCheck;
use lemonfiber_core::doctor::{Category, Check, Verdict};
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_manifest::{Bind, Manifest, Service};

/// The project these containers belong to.
const PROJECT: &str = "lemonfiber";

/// A service the stack declares in one tier.
///
/// Read back through the manifest parser rather than built as a struct, so what this
/// test calls a tier is what the stack's own file means by one.
fn service(id: &str, bind: Bind) -> Service {
    let tier = match bind {
        Bind::Loopback => "loopback",
        Bind::Lan => "lan",
    };
    let written = format!(
        "schema_version = 1\nstack_version = \"0.1.0\"\nmin_cli_version = \"0.1.0\"\n\n\
         [[profile]]\nid = \"tv\"\nname = \"Television\"\n\
         description = \"Television\"\n\n\
         [[service]]\nid = \"{id}\"\nname = \"{id}\"\nprofile = \"tv\"\n\
         image = \"example/{id}\"\ntag = \"1.0.0\"\nport = 8989\nbind = \"{tier}\"\n\
         criticality = \"core\"\nlicense = \"GPL-3.0-only\"\n\
         upstream = \"https://example.invalid/{id}\"\nlast_release = \"2026-01-01\"\n\
         describes = \"Does a thing\"\nwithout_it = \"Do the thing yourself\"\n"
    );
    let read = Manifest::from_toml(&written)
        .ok()
        .and_then(|manifest| manifest.services.into_iter().next());
    let Some(service) = read else {
        unreachable!("a manifest this test wrote is one the parser reads: {written}")
    };
    service
}

/// What the check found, as the verdict on its one finding.
async fn found(engine: Reporting, services: &[Service]) -> Vec<Verdict> {
    BindingsCheck::new(Arc::new(engine), PROJECT.to_owned(), services)
        .run()
        .await
        .into_iter()
        .map(|finding| finding.verdict)
        .collect()
}

/// An engine holding these services, publishing these addresses.
fn engine(services: &[&str], published: &[(&str, &str, u16)]) -> Reporting {
    Reporting::holding(
        services,
        lemonfiber_ports::docker::Lifecycle::Running,
        lemonfiber_ports::docker::Health::Healthy,
    )
    .publishing(published)
}

#[tokio::test]
async fn an_admin_service_answering_this_machine_alone_is_what_the_policy_says() {
    let verdicts = found(
        engine(&["sonarr"], &[("sonarr", "127.0.0.1", 8989)]),
        &[service("sonarr", Bind::Loopback)],
    )
    .await;
    assert!(
        matches!(verdicts.as_slice(), [Verdict::Pass { .. }]),
        "{verdicts:?}"
    );
}

#[tokio::test]
async fn an_admin_service_answering_every_interface_is_reported_and_named() {
    let verdicts = found(
        engine(&["sonarr"], &[("sonarr", "0.0.0.0", 8989)]),
        &[service("sonarr", Bind::Loopback)],
    )
    .await;
    let said = format!("{verdicts:?}");
    assert!(matches!(verdicts.as_slice(), [Verdict::Fail(_)]), "{said}");
    assert!(said.contains("8989"), "{said}");
    assert!(
        said.contains("every interface this machine has"),
        "a wildcard is said in words as well as in numbers: {said}"
    );
}

/// The same policy, on the other family.
///
/// The failure this catches is the one that reads as enforced: a rule applied to IPv4
/// and silently absent on IPv6 leaves an operator checking the half that was written
/// down. The engine reports one entry per address, so both are held separately.
#[tokio::test]
async fn the_same_service_answering_every_interface_on_ipv6_is_reported_too() {
    let verdicts = found(
        engine(&["sonarr"], &[("sonarr", "::", 8989)]),
        &[service("sonarr", Bind::Loopback)],
    )
    .await;
    assert!(
        matches!(verdicts.as_slice(), [Verdict::Fail(_)]),
        "{verdicts:?}"
    );

    // And the loopback of that family is as allowed as the other one's.
    let allowed = found(
        engine(&["sonarr"], &[("sonarr", "::1", 8989)]),
        &[service("sonarr", Bind::Loopback)],
    )
    .await;
    assert!(
        matches!(allowed.as_slice(), [Verdict::Pass { .. }]),
        "{allowed:?}"
    );
}

/// A service published on both families and wrong on one of them is one finding for
/// the one that is wrong, not a pass for the pair.
#[tokio::test]
async fn one_family_bound_correctly_does_not_excuse_the_other() {
    let verdicts = found(
        engine(
            &["sonarr"],
            &[("sonarr", "127.0.0.1", 8989), ("sonarr", "::", 8989)],
        ),
        &[service("sonarr", Bind::Loopback)],
    )
    .await;
    assert!(
        matches!(verdicts.as_slice(), [Verdict::Fail(_)]),
        "{verdicts:?}"
    );
}

/// An address off this machine is said as the address it is.
///
/// A wildcard is worth saying in words because it is not an address anything is at.
/// One that is one is not, and saying it twice would be this check having an opinion
/// about a number the operator can read.
#[tokio::test]
async fn an_address_that_is_an_address_is_said_as_one() {
    let verdicts = found(
        engine(&["sonarr"], &[("sonarr", "192.168.1.10", 8989)]),
        &[service("sonarr", Bind::Loopback)],
    )
    .await;
    let said = format!("{verdicts:?}");
    assert!(matches!(verdicts.as_slice(), [Verdict::Fail(_)]), "{said}");
    assert!(said.contains("192.168.1.10"), "{said}");
    assert!(!said.contains("every interface"), "{said}");
}

/// A household service is meant to be reachable, so reaching it is not a fault.
#[tokio::test]
async fn a_household_service_on_every_interface_is_what_it_is_for() {
    let verdicts = found(
        engine(&["jellyfin"], &[("jellyfin", "0.0.0.0", 8096)]),
        &[service("jellyfin", Bind::Lan)],
    )
    .await;
    assert!(
        matches!(verdicts.as_slice(), [Verdict::Pass { .. }]),
        "{verdicts:?}"
    );
}

/// Two services wrong is two findings, because each is a separate thing to put right.
#[tokio::test]
async fn each_service_that_is_wrong_is_its_own_finding() {
    let verdicts = found(
        engine(
            &["sonarr", "radarr"],
            &[("sonarr", "0.0.0.0", 8989), ("radarr", "0.0.0.0", 7878)],
        ),
        &[
            service("sonarr", Bind::Loopback),
            service("radarr", Bind::Loopback),
        ],
    )
    .await;
    assert_eq!(verdicts.len(), 2, "{verdicts:?}");
    assert!(
        verdicts
            .iter()
            .all(|verdict| matches!(verdict, Verdict::Fail(_))),
        "{verdicts:?}"
    );
}

/// A service the stack declares no tier for is passed over rather than guessed at.
///
/// A rule invented here would be a second opinion about a question the stack already
/// answers, and the two would disagree the day one of them changed.
#[tokio::test]
async fn a_service_the_stack_declares_no_tier_for_is_not_judged() {
    let verdicts = found(
        engine(&["something"], &[("something", "0.0.0.0", 9999)]),
        &[],
    )
    .await;
    assert!(
        matches!(verdicts.as_slice(), [Verdict::Pass { .. }]),
        "{verdicts:?}"
    );
}

/// A stack that is down publishes nothing, which is not the same as one bound right.
#[tokio::test]
async fn a_stack_publishing_nothing_is_skipped_rather_than_passed() {
    let verdicts = found(
        engine(&["sonarr"], &[]),
        &[service("sonarr", Bind::Loopback)],
    )
    .await;
    assert!(
        matches!(verdicts.as_slice(), [Verdict::Skipped { .. }]),
        "{verdicts:?}"
    );
}

/// An engine that will not say is never a pass.
#[tokio::test]
async fn an_engine_that_will_not_say_leaves_this_unestablished() {
    let verdicts = found(Reporting::absent(), &[service("sonarr", Bind::Loopback)]).await;
    assert!(
        matches!(verdicts.as_slice(), [Verdict::Unverified { .. }]),
        "{verdicts:?}"
    );
}

/// The check says which family of diagnosis it belongs to, so a run can be narrowed
/// to it.
#[test]
fn it_belongs_to_the_family_a_run_can_be_narrowed_to() {
    let check = BindingsCheck::new(Arc::new(Reporting::absent()), PROJECT.to_owned(), &[]);
    assert_eq!(check.category(), Category::Network);
}
