//! Nothing the household is handed names a service that can change the stack.
//!
//! The front-door answer is the one thing this product produces *for* the people in
//! the house rather than for the operator: it is what is read out over the phone,
//! what a dashboard shows and what an invitation will carry. Everything on it is
//! therefore household-facing, and the rule is that a household-facing surface
//! never leads to an administrative one.
//!
//! Held here rather than left to the three renderings of it, because every one of
//! them draws from this answer and none of them has anything else to draw from — a
//! terminal, a browser and a screen all render these fields, so a name that reaches
//! this reaches all three, and a name that cannot reach it reaches none.
//!
//! Two properties, and the second is the one that is easy to lose. Nothing is
//! *named*: the admin tier is not on the answer at all, which the register's
//! default already does and which a filter dropped from the list beside the door
//! would undo in silence. And nothing but the door is *linkable*: what stands beside
//! it is carried as a name and a reason with no address on it, so a reader who
//! wanted to try one has nothing to try. A list of addresses to everything on the
//! network would satisfy the first and break the second.
//!
//! Read over the stack this repository ships rather than over a stack written here,
//! because what counts as the admin tier is the stack's own answer and a fixture
//! that stated it would be this test agreeing with itself.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::common::stack::{frozen_day, stack};
use lemonfiber_contract::capabilities::{media::serve, stream::guard};
use lemonfiber_core::app::{dispatch, Command, Outcome};
use lemonfiber_core::config::Settings;
use lemonfiber_core::model::FrontDoorReport;
use lemonfiber_fixtures::ports::Renamed;
use lemonfiber_fixtures::support::Reporting;
use lemonfiber_manifest::{Bind, Manifest, Service};
use lemonfiber_ports::docker::{Health, Lifecycle};

/// What that stack declares.
fn declared() -> Manifest {
    let read = stack().checked_manifest(frozen_day()).ok();
    let Some(manifest) = read else {
        unreachable!("the stack this repository ships is one its own parser reads")
    };
    assert!(
        manifest.services.len() > 5,
        "a manifest this short means the wrong file was read"
    );
    manifest
}

/// The services the household tier does not publish — the ones an arrival could
/// change this stack from.
fn administrative(manifest: &Manifest) -> Vec<&Service> {
    manifest
        .services
        .iter()
        .filter(|service| service.bind != Some(Bind::Lan) && !behind_the_door(manifest, service))
        .collect()
}

/// Whether a service is kept to this machine only because the guard in front of the
/// library stands in front of it: the service the guard's ask for `media.serve` is filled
/// by, which the household reaches at the guard under its own name, and which is no way
/// to change the stack.
fn behind_the_door(manifest: &Manifest, service: &Service) -> bool {
    let guards: Vec<&str> = manifest
        .services
        .iter()
        .filter(|one| {
            one.bind == Some(Bind::Lan) && one.provides.iter().any(|it| it == guard::CAPABILITY)
        })
        .map(|one| one.id.as_str())
        .collect();
    manifest.wirings.iter().any(|wiring| {
        guards.contains(&wiring.by.as_str())
            && wiring.asks.as_deref() == Some(serve::CAPABILITY)
            && wiring.filled_by.as_deref() == Some(service.id.as_str())
    })
}

/// The answer, over a stack whose household services are up with every part the bundle
/// installs recorded, on a machine that says what it is called.
async fn answered(chose: Option<&str>) -> FrontDoorReport {
    static ASKED: AtomicU32 = AtomicU32::new(0);
    let nth = ASKED.fetch_add(1, Ordering::Relaxed);
    let at = lemonfiber_fixtures::scratch::Scratch::named(&format!("handed-{nth}")).kept();
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::create_dir_all(&at);
    let env = at.join(".env");
    let _ = std::fs::write(&env, "");
    let settings = Settings {
        front_door: chose.map(str::to_owned),
        env_file: Some(env),
        ..Settings::default()
    };
    let ctx = lemonfiber_testing::a_context()
        .engine(Arc::new(Reporting::holding(
            &["seerr", "door"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .clock(Arc::new(lemonfiber_adapters::System))
        .settings(settings)
        .build()
        .with_site(Renamed::called(Some("kitchen-nas")));
    lemonfiber_testing::adapted::parts_recorded(&ctx);
    let answer = dispatch(Command::FrontDoor, &ctx).await.ok();
    let Some(Outcome::FrontDoor(report)) = answer else {
        unreachable!("the front-door command answers with a front-door report: {answer:?}")
    };
    report
}

/// Everything the answer would put in front of somebody, as one body of text.
///
/// The serialised answer rather than a chosen handful of its fields, so a field
/// added later is inside this without anybody having to remember to add it — which
/// is the failure a guard listing fields by name would have.
fn everything(report: &FrontDoorReport) -> String {
    serde_json::to_string(report).unwrap_or_default()
}

#[tokio::test]
async fn nothing_the_household_is_handed_names_a_service_that_can_change_the_stack() {
    let manifest = declared();
    let report = answered(None).await;
    let shown = everything(&report).to_lowercase();

    let named: Vec<&str> = administrative(&manifest)
        .iter()
        .filter(|service| {
            shown.contains(&service.id.to_lowercase())
                || shown.contains(&service.name.to_lowercase())
        })
        .map(|service| service.id.as_str())
        .collect();
    assert!(
        named.is_empty(),
        "these answer this machine alone and the household's own answer names them: {named:?}"
    );
}

#[tokio::test]
async fn what_the_household_can_reach_is_still_named_so_the_guard_above_means_something() {
    // The other half of the pair. An answer that named nothing at all would pass the
    // guard above and say nothing, so what the household *can* reach is asserted to
    // be on it — including the index over every service, which is named there in
    // order to be refused by name.
    let report = answered(None).await;
    let beside: Vec<&str> = report
        .beside
        .iter()
        .map(|beside| beside.service.as_str())
        .collect();
    assert!(beside.contains(&"Homepage"), "{beside:?}");
    assert!(beside.contains(&"Jellyfin"), "{beside:?}");
    assert_eq!(report.service.as_deref(), Some("Seerr"));
}

#[tokio::test]
async fn only_what_faces_the_household_is_handed_over() {
    // Naming a service and handing over the address of one are different acts, and
    // the second is what a link is. A member may be handed the way to somewhere meant
    // for them — the library, a shelf — because arriving there is the point of
    // naming it. The rest are named in order to be refused **by name**, and an
    // address on one of those would be that refusal undone.
    let report = answered(None).await;
    assert!(report
        .address
        .is_some_and(|address| address.url.contains("kitchen-nas.local:5055")));

    let handed: Vec<&str> = report
        .beside
        .iter()
        .filter(|beside| beside.address.is_some())
        .map(|beside| beside.service.as_str())
        .collect();
    assert!(
        handed.contains(&"Jellyfin"),
        "the library is somewhere a member is meant to arrive: {handed:?}"
    );

    // The index over every service says of itself that it includes the ones nobody
    // in the house should learn exist. It is the one this rule is really about.
    let withheld: Vec<&str> = report
        .beside
        .iter()
        .filter(|beside| beside.address.is_none())
        .map(|beside| beside.service.as_str())
        .collect();
    assert!(
        withheld.contains(&"Homepage") && withheld.contains(&"Caddy"),
        "a service the household is not sent to was handed over anyway: {withheld:?}"
    );

    // And the reason a service is not the door is still prose, never a link.
    assert!(report
        .beside
        .iter()
        .all(|beside| !beside.because.contains("http")));
}

#[tokio::test]
async fn a_setting_cannot_talk_the_answer_into_naming_an_administrative_service() {
    // The one way a name from outside reaches the door. Every service the stack keeps
    // to this machine is put to it in turn, because a rule that held for the one
    // somebody thought of is not a rule.
    let manifest = declared();
    let kept_to_this_machine = administrative(&manifest);
    // The filter is what makes this a rule about administrative services, and it is
    // also the way this stops being a rule at all: a stack where every service became
    // reachable would run the sweep below over nothing and report that none of them
    // can be named.
    assert!(
        !kept_to_this_machine.is_empty(),
        "no service is kept to this machine, so nothing was put to the door"
    );
    for service in kept_to_this_machine {
        let report = answered(Some(&service.id)).await;
        assert_eq!(
            report.service.as_deref(),
            Some("Seerr"),
            "{} was named and obeyed",
            service.id
        );
        assert!(
            report.meaning.contains(&service.id),
            "{} was refused in silence",
            service.id
        );
        assert!(
            report
                .beside
                .iter()
                .all(|beside| beside.service != service.name),
            "{} is listed as somewhere the household can reach",
            service.id
        );
    }
}

#[tokio::test]
async fn the_index_over_every_service_cannot_be_named_as_the_door_either() {
    // It is published to the household, so the tier alone does not refuse it — and it
    // is a page of links to every administrative service this stack runs, which is
    // exactly what the household must not be handed.
    let report = answered(Some("homepage")).await;
    assert_eq!(report.service.as_deref(), Some("Seerr"));
    assert!(report.meaning.contains("homepage"), "{}", report.meaning);
}
