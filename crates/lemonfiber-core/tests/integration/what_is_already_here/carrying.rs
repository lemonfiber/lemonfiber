//! Carrying records across from an existing setup.
//!
//! Lemonfiber's own curator is asked through the adapter in front of it, and theirs
//! through the adapter of the same first-party plugin lent to it, both answering
//! `library.curate` over one transport that tells them apart by the port asked.

use super::common::stack::project;
use super::{
    both_stacks, carrying, importing, importing_over, lent, of_sort, over, somebody_elses, stacks,
    taskmaster, two_stacks, THEIR_KEY,
};
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::service::Carried;
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::http::Fake;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::serving::Serves;
use lemonfiber_fixtures::support::{Reporting, SeedFs};
use lemonfiber_ports::serving::Standing;
use lemonfiber_testing::curators::{curating, refusing, taken, told, unavailable, TELEVISION};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// What a report names as not carried.
fn not_carried(found: Option<lemonfiber_core::model::ImportReport>) -> Vec<String> {
    found.map_or_else(Vec::new, |read| {
        read.not_carried.into_iter().map(|one| one.what).collect()
    })
}

/// Why the first thing a report names as not carried was not.
fn because(found: Option<lemonfiber_core::model::ImportReport>) -> String {
    found.map_or_else(String::new, |read| {
        read.not_carried
            .first()
            .map(|one| one.because.clone())
            .unwrap_or_default()
    })
}

/// A machine holding both stacks, its files read from `files`.
fn importing_with(files: SeedFs, http: Arc<Fake>) -> lemonfiber_core::app::Ctx {
    importing_over(
        both_stacks().0,
        Source::External(project()),
        Arc::new(files),
        http,
    )
}

#[tokio::test]
async fn carrying_unconfirmed_names_what_would_travel_and_writes_nothing() {
    let http = two_stacks();
    let found = carrying(&importing(Arc::clone(&http)), false).await;
    let named: Vec<String> = found
        .map(|read| read.would_carry.into_iter().map(|one| one.name).collect())
        .unwrap_or_default();
    assert_eq!(named, vec!["Taskmaster".to_owned()], "what would travel");
    assert!(
        !http.asked_for(curating(TELEVISION, "carry")),
        "a rehearsal wrote to a service"
    );
}

/// The record their copy holds is handed to ours whole, its profile and its folder by
/// name and path, and never written to theirs.
#[tokio::test]
async fn a_carried_record_is_handed_to_ours_as_theirs_holds_it() {
    let http = two_stacks();
    let found = carrying(&importing(Arc::clone(&http)), true).await;
    let carried: Vec<String> = found
        .map(|read| read.carried.into_iter().map(|one| one.name).collect())
        .unwrap_or_default();
    assert_eq!(carried, vec!["Taskmaster".to_owned()], "what travelled");
    assert_eq!(
        told::<Carried>(&http.requests(), TELEVISION, "carry", "item"),
        vec![taskmaster()]
    );
    assert!(!http.asked_for(lent("carry")), "theirs was written to");
}

/// A stack that could not be read is one the survey has already refused, so nothing is
/// carried and nothing is claimed.
#[tokio::test]
async fn carrying_out_of_a_machine_that_could_not_be_read_carries_nothing() {
    let (engine, _) = both_stacks();
    let nowhere = Source::External(Path::new("/nowhere-at-all"));
    let ctx = importing_over(
        engine,
        nowhere,
        Arc::new(SeedFs::keyed(Some(THEIR_KEY))),
        two_stacks(),
    );
    let found = carrying(&ctx, true).await;
    let refused = found.and_then(|read| read.refusal);
    assert!(refused.is_some(), "refused rather than silently empty");
}

/// A service publishing nothing is one there is no way to reach a copy of.
#[tokio::test]
async fn a_service_with_no_way_in_is_named_rather_than_passed_over() {
    let unreachable = Reporting::holding(&["sonarr"], Lifecycle::Running, Health::Healthy)
        .belonging_to("media")
        .mounting(&[PathBuf::from("/their/sonarr")]);
    let ctx = importing_over(
        unreachable,
        Source::External(project()),
        Arc::new(SeedFs::keyed(Some(THEIR_KEY))),
        two_stacks(),
    );
    let found = carrying(&ctx, false).await;
    assert_eq!(
        not_carried(found),
        vec!["sonarr".to_owned()],
        "named rather than dropped"
    );
}

/// A service lemonfiber does not run holds nothing this knows how to carry.
#[tokio::test]
async fn a_service_we_do_not_run_holds_nothing_to_carry() {
    let engine = Reporting::holding(&["sonarr", "ombi"], Lifecycle::Running, Health::Healthy)
        .belonging_to("media")
        .publishing(&[("sonarr", "0.0.0.0", 18989)])
        .mounting(&[PathBuf::from("/their/sonarr")]);
    let ctx = importing_over(
        engine,
        Source::External(project()),
        Arc::new(SeedFs::keyed(Some(THEIR_KEY))),
        two_stacks(),
    );
    let found = carrying(&ctx, false).await;
    let named: Vec<String> = found
        .map(|read| read.would_carry.into_iter().map(|one| one.name).collect())
        .unwrap_or_default();
    assert_eq!(named, vec!["Taskmaster".to_owned()], "only what we run");
}

/// A service that will not take a record says so, rather than the import claiming it.
#[tokio::test]
async fn a_record_the_service_refuses_is_reported_rather_than_counted() {
    let refused = Fake::by_call(stacks(&[taskmaster()], &[], refusing()));
    let found = carrying(&importing(refused), true).await;
    let carried = found
        .as_ref()
        .map(|read| read.carried.len())
        .unwrap_or_default();
    assert_eq!(carried, 0, "nothing was counted as carried");
    assert_eq!(
        not_carried(found),
        vec!["Taskmaster".to_owned()],
        "named as not carried"
    );
}

/// An adapter lent to their copy that never comes ready is a service nothing was carried
/// out of, and it is taken away again.
#[tokio::test]
async fn a_service_whose_lent_adapter_never_comes_ready_is_named() {
    let serves = Arc::new(Serves::on(super::LENT).standing(Standing::Exited(1)));
    let mut ctx = importing(Fake::silent());
    ctx.seams.serving = Arc::clone(&serves) as Arc<dyn lemonfiber_ports::serving::Serving>;
    let found = carrying(&ctx, false).await;
    assert_eq!(
        not_carried(found),
        vec!["sonarr".to_owned()],
        "named rather than silent"
    );
    assert_eq!(
        serves.removed().len(),
        1,
        "the lent adapter was left running"
    );
}

/// A service that has written no key yet is one their copy cannot be opened for.
#[tokio::test]
async fn a_service_whose_key_cannot_be_read_is_named_rather_than_carried_from() {
    let ctx = importing_with(SeedFs::keyed(None), two_stacks());
    let said = because(carrying(&ctx, false).await);
    assert!(said.contains("has written no key"), "{said}");
}

/// Profiles that read and records that do not is a service half-answering, and the
/// import says which half.
#[tokio::test]
async fn a_service_whose_records_will_not_read_is_named_for_that() {
    let mut routes = vec![(
        curating(TELEVISION, "records"),
        Some(of_sort("indexer")),
        vec![unavailable()],
    )];
    routes.extend(stacks(&[taskmaster()], &[], taken()));
    let ctx = importing_with(SeedFs::keyed(Some(THEIR_KEY)), Fake::by_call(routes));
    let said = because(carrying(&ctx, false).await);
    assert!(said.contains("could not be read"), "{said}");
}

/// Ours that will not say what quality profiles it holds is a service nothing can be
/// matched into, and the import says so.
#[tokio::test]
async fn ours_that_will_not_say_its_profiles_is_named_for_that() {
    let mut routes = vec![(
        curating(TELEVISION, "quality_profiles"),
        None,
        vec![unavailable()],
    )];
    routes.extend(stacks(&[taskmaster()], &[], taken()));
    let ctx = importing_with(SeedFs::keyed(Some(THEIR_KEY)), Fake::by_call(routes));
    let said = because(carrying(&ctx, false).await);
    assert!(
        said.ends_with("its quality profiles could not be read"),
        "{said}"
    );
}

/// A machine that could not be looked at is one there is nothing to carry out of, and
/// saying so is different from saying the two stacks agree.
#[tokio::test]
async fn carrying_from_a_machine_that_could_not_be_looked_at_says_so() {
    let refused = Pulled::unreachable("no daemon here");
    let ctx = over(somebody_elses(), refused, Source::External(project()));
    let found = carrying(&ctx, true).await;
    let said = found.and_then(|read| read.refusal).unwrap_or_default();
    assert!(said.contains("could not be read"), "{said}");
}

/// Having looked and found no stack of ours is a different answer from not having
/// looked, and an import says which.
#[tokio::test]
async fn carrying_from_a_machine_holding_nothing_of_ours_says_there_is_no_setup() {
    let images = Pulled::holding(vec![Pulled::image("a-database:17", 400, &["shop"])]);
    let engine =
        Reporting::holding(&["postgres"], Lifecycle::Running, Health::Healthy).belonging_to("shop");
    let ctx = over(engine, images, Source::External(project()));
    let said = carrying(&ctx, true)
        .await
        .and_then(|read| read.refusal)
        .unwrap_or_default();
    assert!(said.contains("no single setup here"), "{said}");
}

/// Two stacks already holding the same records is a state of its own: nothing was
/// carried because there was nothing to carry, which is not the same as an import that
/// carried nothing because something went wrong.
#[tokio::test]
async fn two_stacks_that_already_agree_come_to_nothing_changing() {
    let agreeing = Fake::by_call(stacks(&[taskmaster()], &[taskmaster()], taken()));
    let found = carrying(&importing(agreeing), true).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Unchanged),
        "{found:?}"
    );
}
