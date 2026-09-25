//! Carrying records across from an existing setup.

use super::common::stack::project;
use super::{both_stacks, carrying, importing, over, somebody_elses, two_stacks};
use lemonfiber_core::app::Ctx;
use lemonfiber_core::config::Settings;
use lemonfiber_core::ports::docker::{Health, Lifecycle};
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::reconfigure::Stance;
use lemonfiber_core::stack::Source;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{Reporting, SeedFs};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[tokio::test]
async fn carrying_unconfirmed_names_what_would_travel_and_writes_nothing() {
    let http = two_stacks();
    let found = carrying(&importing(Arc::clone(&http)), false).await;
    let named: Vec<String> = found
        .map(|read| read.would_carry.into_iter().map(|one| one.name).collect())
        .unwrap_or_default();
    assert_eq!(named, vec!["Taskmaster".to_owned()], "what would travel");

    let posted = http
        .requests()
        .into_iter()
        .any(|request| request.method == Method::Post);
    assert!(!posted, "a rehearsal wrote to a service");
}

/// The assertion the whole design turns on: two stacks number their own profiles, so a
/// record carried with the old number would follow whatever happened to be first here.
#[tokio::test]
async fn a_carried_record_follows_this_stacks_own_profile_not_the_number_it_had() {
    let http = two_stacks();
    let found = carrying(&importing(Arc::clone(&http)), true).await;
    let carried: Vec<String> = found
        .map(|read| read.carried.into_iter().map(|one| one.name).collect())
        .unwrap_or_default();
    assert_eq!(carried, vec!["Taskmaster".to_owned()], "what travelled");

    let body = http
        .requests()
        .into_iter()
        .find(|request| request.method == Method::Post)
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(body.contains("\"qualityProfileId\":7"), "remapped: {body}");
    assert!(
        !body.contains("\"qualityProfileId\":1"),
        "not theirs: {body}"
    );
    assert!(
        !body.contains("\"id\":5"),
        "its id there is not its id here: {body}"
    );
}

/// A machine holding both stacks, with the pieces a caller wants to vary.
fn importing_over(
    engine: Reporting,
    stack: Source,
    http: Arc<lemonfiber_fixtures::http::Fake>,
) -> Ctx {
    let images = Pulled::holding(vec![Pulled::image(
        "lscr.io/linuxserver/sonarr:4.0.15",
        400,
        &["media"],
    )]);
    lemonfiber_testing::a_context()
        .engine(Arc::new(engine))
        .filesystem(Arc::new(SeedFs::keyed(
            Some("<Config><ApiKey>the-key</ApiKey></Config>"),
            None,
        )))
        .over(stack)
        .settings(Settings {
            project: "lemonfiber".to_owned(),
            stack_dir: Some(PathBuf::from("/srv/lemonfiber")),
            ..Settings::default()
        })
        .build()
        .with_images(images)
        .with_http(http)
}

/// A stack that could not be read is one the survey has already refused, so nothing is
/// carried and nothing is claimed.
#[tokio::test]
async fn carrying_out_of_a_machine_that_could_not_be_read_carries_nothing() {
    let (engine, _) = both_stacks();
    let nowhere = Source::External(Path::new("/nowhere-at-all"));
    let ctx = importing_over(engine, nowhere, two_stacks());
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
    let ctx = importing_over(unreachable, Source::External(project()), two_stacks());
    let found = carrying(&ctx, false).await;
    let named = found.map_or_else(Vec::new, |read| {
        read.not_carried.into_iter().map(|one| one.what).collect()
    });
    assert_eq!(
        named,
        vec!["sonarr".to_owned()],
        "named rather than dropped"
    );
}

/// A service lemonfiber does not run holds nothing this knows how to carry.
#[tokio::test]
async fn a_service_we_do_not_run_holds_nothing_to_carry() {
    let engine = Reporting::holding(&["sonarr", "ombi"], Lifecycle::Running, Health::Healthy)
        .belonging_to("media")
        .publishing(&[("sonarr", "127.0.0.1", 18989)])
        .mounting(&[PathBuf::from("/their/sonarr")]);
    let ctx = importing_over(engine, Source::External(project()), two_stacks());
    let found = carrying(&ctx, false).await;
    let named: Vec<String> = found
        .map(|read| read.would_carry.into_iter().map(|one| one.name).collect())
        .unwrap_or_default();
    assert_eq!(named, vec!["Taskmaster".to_owned()], "only what we run");
}

/// A service that will not take a record says so, rather than the import claiming it.
#[tokio::test]
async fn a_record_the_service_refuses_is_reported_rather_than_counted() {
    use lemonfiber_fixtures::http::{Answer, Fake};
    let refusing = Fake::by_route(vec![
        (
            Method::Get,
            "18989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":1,"name":"HD"}]"#),
        ),
        (
            Method::Get,
            "18989/api/v3/series",
            Answer::reply(
                200,
                r#"[{"id":5,"title":"Taskmaster","qualityProfileId":1}]"#,
            ),
        ),
        (
            Method::Get,
            "18989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
        (
            Method::Get,
            ":8989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":7,"name":"HD"}]"#),
        ),
        (Method::Get, ":8989/api/v3/series", Answer::reply(200, "[]")),
        (
            Method::Get,
            ":8989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
        (
            Method::Post,
            ":8989/api/v3/series",
            Answer::reply(500, "no"),
        ),
    ]);
    let (engine, _) = both_stacks();
    let ctx = importing_over(engine, Source::External(project()), refusing);

    let found = carrying(&ctx, true).await;
    let carried = found
        .as_ref()
        .map(|read| read.carried.len())
        .unwrap_or_default();
    assert_eq!(carried, 0, "nothing was counted as carried");
    let named = found.map_or_else(Vec::new, |read| {
        read.not_carried.into_iter().map(|one| one.what).collect()
    });
    assert_eq!(named, vec!["Taskmaster".to_owned()], "named as not carried");
}

/// Neither copy answering is a service nothing was carried out of.
#[tokio::test]
async fn a_service_neither_copy_answers_for_is_named() {
    let (engine, _) = both_stacks();
    let silent = lemonfiber_fixtures::http::Fake::silent();
    let ctx = importing_over(engine, Source::External(project()), silent);
    let found = carrying(&ctx, false).await;
    let named = found.map_or_else(Vec::new, |read| {
        read.not_carried.into_iter().map(|one| one.what).collect()
    });
    assert_eq!(named, vec!["sonarr".to_owned()], "named rather than silent");
}

/// A machine holding both stacks, with a filesystem the caller chooses.
fn importing_with(files: Arc<SeedFs>, http: Arc<lemonfiber_fixtures::http::Fake>) -> Ctx {
    let (engine, images) = both_stacks();
    lemonfiber_testing::a_context()
        .engine(Arc::new(engine))
        .filesystem(files)
        .settings(Settings {
            project: "lemonfiber".to_owned(),
            stack_dir: Some(PathBuf::from("/srv/lemonfiber")),
            ..Settings::default()
        })
        .build()
        .with_images(images)
        .with_http(http)
}

/// A service that has written no key yet is one neither copy can be opened for.
#[tokio::test]
async fn a_service_whose_key_cannot_be_read_is_named_rather_than_carried_from() {
    let keyless = Arc::new(SeedFs::keyed(None, None));
    let ctx = importing_with(keyless, two_stacks());
    let found = carrying(&ctx, false).await;
    let said = found.map_or_else(String::new, |read| {
        read.not_carried
            .first()
            .map(|one| one.because.clone())
            .unwrap_or_default()
    });
    assert!(said.contains("could not be reached"), "{said}");
}

/// Profiles that read and records that do not is a service half-answering, and the
/// import says which half.
#[tokio::test]
async fn a_service_whose_records_will_not_read_is_named_for_that() {
    use lemonfiber_fixtures::http::{Answer, Fake};
    let partial = Fake::by_route(vec![
        (
            Method::Get,
            "/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":7,"name":"HD"}]"#),
        ),
        (Method::Get, "/api/v3/indexer", Answer::reply(500, "no")),
    ]);
    let ctx = importing_with(
        Arc::new(SeedFs::keyed(
            Some("<Config><ApiKey>the-key</ApiKey></Config>"),
            None,
        )),
        partial,
    );
    let found = carrying(&ctx, false).await;
    let said = found.map_or_else(String::new, |read| {
        read.not_carried
            .first()
            .map(|one| one.because.clone())
            .unwrap_or_default()
    });
    assert!(said.contains("could not be read"), "{said}");
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
    use lemonfiber_fixtures::http::{Answer, Fake};
    let held = r#"[{"id":5,"title":"Taskmaster","qualityProfileId":1}]"#;
    let agreeing = Fake::by_route(vec![
        (
            Method::Get,
            "18989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":1,"name":"HD"}]"#),
        ),
        (Method::Get, "18989/api/v3/series", Answer::reply(200, held)),
        (
            Method::Get,
            "18989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
        (
            Method::Get,
            ":8989/api/v3/qualityprofile",
            Answer::reply(200, r#"[{"id":7,"name":"HD"}]"#),
        ),
        (Method::Get, ":8989/api/v3/series", Answer::reply(200, held)),
        (
            Method::Get,
            ":8989/api/v3/indexer",
            Answer::reply(200, "[]"),
        ),
    ]);
    let (engine, _) = both_stacks();
    let ctx = importing_over(engine, Source::External(project()), agreeing);

    let found = carrying(&ctx, true).await;
    assert_eq!(
        found.as_ref().map(|read| read.stance),
        Some(Stance::Unchanged),
        "{found:?}"
    );
}
