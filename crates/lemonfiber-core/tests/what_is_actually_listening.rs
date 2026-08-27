//! What the stack is actually listening on, held against what it is meant to.
//!
//! Driven through the container-engine port, because the whole point of the check is
//! that it reads the runtime's own account rather than the files that asked for it —
//! and the way to prove that is to make the two disagree.

use std::sync::Arc;

use lemonfiber_core::doctor::bindings::BindingsCheck;
use lemonfiber_core::doctor::{Category, Check, Verdict};
use lemonfiber_core::platform::Environment;
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
///
/// Asked as a platform whose engine is behind a virtual machine, so what the
/// existing cases are about — where a service is listening — is not mixed up with
/// the one that is about how a published port is reached.
async fn found(engine: Reporting, services: &[Service]) -> Vec<Verdict> {
    on(engine, services, Environment::MacOs).await
}

/// The same, on a platform the test names.
async fn on(engine: Reporting, services: &[Service], environment: Environment) -> Vec<Verdict> {
    BindingsCheck::new(
        Arc::new(engine),
        PROJECT.to_owned(),
        services,
        environment,
        &[],
    )
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
    let check = BindingsCheck::new(
        Arc::new(Reporting::absent()),
        PROJECT.to_owned(),
        &[],
        Environment::MacOs,
        &[],
    );
    assert_eq!(check.category(), Category::Network);
}

/// A published port is said to be reached around this machine's own rules, where it is.
///
/// The engine running directly on this machine writes its forwarding rules ahead of
/// the ones a person adds, so a rule written to close one of these ports is not what
/// decides whether it answers. Nothing here reads a firewall — that would be a guess
/// about which of several tools the operator uses — so what is said is the
/// arrangement, and the address is named so an operator can tell which port it is
/// about.
#[tokio::test]
async fn a_published_port_is_said_to_be_reached_around_this_machines_own_rules() {
    let verdicts = on(
        engine(&["jellyfin"], &[("jellyfin", "0.0.0.0", 8096)]),
        &[service("jellyfin", Bind::Lan)],
        Environment::LinuxNative,
    )
    .await;

    let warned: Vec<&Verdict> = verdicts
        .iter()
        .filter(|verdict| matches!(verdict, Verdict::Warn(_)))
        .collect();
    assert_eq!(warned.len(), 1, "{verdicts:?}");
    let said = match warned.first() {
        Some(Verdict::Warn(problem)) => format!("{problem:?}"),
        _ => String::new(),
    };
    assert!(said.contains("jellyfin"), "it names the service: {said}");
    assert!(
        said.contains("every interface"),
        "and what the address means: {said}"
    );
    assert!(
        said.contains("LAN_BIND"),
        "and what does decide, which is the only thing to do about it: {said}"
    );
}

/// It is not said where the arrangement does not hold.
///
/// Docker Desktop puts the engine behind a virtual machine and forwards from a
/// process the host's firewall does see, so macOS, Windows and Desktop-on-Linux are
/// not this case. A warning that fired on every platform would be one nobody could
/// act on, and the three that are not warned are named rather than left to the one
/// that is.
#[tokio::test]
async fn it_is_not_said_where_the_engine_is_behind_a_virtual_machine() {
    for elsewhere in [
        Environment::MacOs,
        Environment::Windows,
        Environment::LinuxDesktop,
    ] {
        let verdicts = on(
            engine(&["jellyfin"], &[("jellyfin", "0.0.0.0", 8096)]),
            &[service("jellyfin", Bind::Lan)],
            elsewhere,
        )
        .await;
        assert!(
            !verdicts
                .iter()
                .any(|verdict| matches!(verdict, Verdict::Warn(_))),
            "{elsewhere:?}: {verdicts:?}"
        );
    }
}

/// And not where nothing is published past this machine.
///
/// A stack whose every port answers loopback alone has nothing a firewall rule would
/// have been written about, so there is nothing to say — the warning is about ports
/// somebody might believe are shut, not about the engine being what it is.
#[tokio::test]
async fn it_is_not_said_where_everything_answers_this_machine_alone() {
    let verdicts = on(
        engine(&["sonarr"], &[("sonarr", "127.0.0.1", 8989)]),
        &[service("sonarr", Bind::Loopback)],
        Environment::LinuxNative,
    )
    .await;
    assert!(
        matches!(verdicts.as_slice(), [Verdict::Pass { .. }]),
        "{verdicts:?}"
    );
}

/// An exposure the operator wrote down stops being a failure and stays reported.
///
/// The two halves of what an acknowledgement is for: it is no longer this product
/// telling somebody they got it wrong, and it is still on the report — because the
/// exposure is real and does not stop being real for having been agreed to. Their
/// own words come back with it, so a diagnosis somebody sends on carries why.
#[tokio::test]
async fn an_exposure_the_operator_wrote_down_is_reported_as_theirs() {
    let said = [(
        "sonarr".to_owned(),
        "it is behind the reverse proxy I already run".to_owned(),
    )];
    let verdicts = BindingsCheck::new(
        Arc::new(engine(&["sonarr"], &[("sonarr", "0.0.0.0", 8989)])),
        PROJECT.to_owned(),
        &[service("sonarr", Bind::Loopback)],
        Environment::MacOs,
        &said,
    )
    .run()
    .await
    .into_iter()
    .map(|finding| finding.verdict)
    .collect::<Vec<_>>();

    let told = match verdicts.as_slice() {
        [Verdict::Warn(problem)] => format!("{problem:?}"),
        other => unreachable!("an acknowledged exposure is a warning, not {other:?}"),
    };
    assert!(told.contains("sonarr"), "{told}");
    assert!(
        told.contains("reverse proxy I already run"),
        "their own words come back: {told}"
    );
}

/// An acknowledgement that gives no reason is not one.
///
/// A name on its own records that somebody clicked past a warning and nothing about
/// why, and the one arrangement worse than no acknowledgement is a malformed one
/// that silences the warning anyway. So a short reason, an empty one and a missing
/// one each leave the exposure reported as a failure.
#[test]
fn an_acknowledgement_that_says_nothing_is_not_one() {
    use lemonfiber_core::config::{env::EnvFile, exposed_from_env, EXPOSED_KEY};

    for written in [
        "sonarr",
        "sonarr=",
        "sonarr=yes",
        "sonarr=ok fine",
        "=a good long reason",
    ] {
        let file = EnvFile::parse(&format!("{EXPOSED_KEY}={written}\n"));
        assert!(
            exposed_from_env(&file).is_empty(),
            "{written:?} is not an acknowledgement"
        );
    }

    let file = EnvFile::parse(&format!(
        "{EXPOSED_KEY}=sonarr=it is behind the proxy I run,radarr=the same is true of this one\n"
    ));
    assert_eq!(exposed_from_env(&file).len(), 2, "and two of them are two");
}

/// A reason given for one service does not excuse another.
#[tokio::test]
async fn saying_so_about_one_service_says_nothing_about_the_next() {
    let said = [(
        "sonarr".to_owned(),
        "it is behind the proxy I run".to_owned(),
    )];
    let verdicts = BindingsCheck::new(
        Arc::new(engine(
            &["sonarr", "radarr"],
            &[("sonarr", "0.0.0.0", 8989), ("radarr", "0.0.0.0", 7878)],
        )),
        PROJECT.to_owned(),
        &[
            service("sonarr", Bind::Loopback),
            service("radarr", Bind::Loopback),
        ],
        Environment::MacOs,
        &said,
    )
    .run()
    .await
    .into_iter()
    .map(|finding| finding.verdict)
    .collect::<Vec<_>>();

    let failed = verdicts
        .iter()
        .filter(|verdict| matches!(verdict, Verdict::Fail(_)))
        .count();
    let warned = verdicts
        .iter()
        .filter(|verdict| matches!(verdict, Verdict::Warn(_)))
        .count();
    assert_eq!((failed, warned), (1, 1), "{verdicts:?}");
}
