//! The agreement each tier needs, and what it removes once given.

use super::*;

/// The same tree, with something of the operator's beside it.
fn and_theirs() -> Vec<Occupant> {
    let mut tree = only_ours();
    tree.push(file("/srv/media/Photographs/2019/a.jpg", 500));
    tree
}

/// The tier that takes the library will not act on a bare yes, and the
/// refusal states what would be destroyed.
#[tokio::test]
async fn the_media_tier_confirmed_without_its_own_agreement_is_refused() {
    let asked = Removing::surveying(Tier::Media).confirmed(true);
    let refused = uninstalled(&a_machine(), asked).await;

    let problem = refused.err();
    assert_eq!(
        problem.as_ref().map(|problem| problem.code),
        Some(NEEDS_AGREEING)
    );
    assert!(
        problem.is_some_and(|problem| problem.summary.contains("2.9 KiB")),
        "the refusal does not state what is at stake"
    );
}

/// An answer given against a machine that has moved since is refused, so a
/// yes cannot be carried from one reading to another.
#[tokio::test]
async fn an_agreement_given_for_another_reading_is_refused() {
    let asked = Removing::surveying(Tier::Media)
        .confirmed(true)
        .agreeing(Some("deadbeef".to_owned()));
    let refused = uninstalled(&a_machine(), asked).await;

    assert!(refused.is_err_and(|problem| problem.code == ANOTHER_READING));
}

/// And a stale answer is refused on a tier that needed none either, so no
/// removal here can be reached by a yes given for something else.
#[tokio::test]
async fn a_stale_agreement_is_refused_on_a_tier_that_did_not_need_one() {
    let asked = Removing::surveying(Tier::Services)
        .confirmed(true)
        .agreeing(Some("deadbeef".to_owned()));
    let refused = uninstalled(&a_machine(), asked).await;

    assert!(refused.is_err_and(|problem| problem.code == ANOTHER_READING));
}

/// Answered by the name the reading printed, the library goes.
#[tokio::test]
async fn the_media_tier_answered_by_the_name_it_printed_removes_the_library() {
    let eraser = Erasing::willing();
    let ctx = a_machine().erasing(Arc::clone(&eraser) as Arc<dyn Eraser>);

    let removal = confirmed(&ctx, Tier::Media).await;

    assert!(
        removal.is_some_and(|removal| matches!(removal, Removal::Complete { .. })),
        "the removal did not complete"
    );
    assert_eq!(eraser.asked(), vec![PathBuf::from(ROOT)]);
}

/// With nothing of the operator's beneath it, the data location is offered
/// as one tree — the contrast the next case turns on.
#[tokio::test]
async fn a_data_location_holding_only_the_stacks_own_files_is_offered_whole() {
    let manifest = read(&a_machine(), Tier::Media).await;

    assert_eq!(
        manifest.as_ref().map(|manifest| manifest.foreign.len()),
        Some(0)
    );
    assert_eq!(
        manifest.map(|manifest| going(&manifest)),
        Some(vec![ROOT.to_owned()])
    );
}

/// One photograph beside the library stops the whole tree from being taken,
/// and the stack's own directories are offered one at a time instead.
#[tokio::test]
async fn something_of_the_operators_beside_the_library_prevents_a_blanket_removal() {
    let ctx = a_machine().surveying(Walking::holding(and_theirs()));
    let manifest = read(&ctx, Tier::Media).await;

    assert_eq!(
        manifest.as_ref().map(|manifest| manifest
            .foreign
            .iter()
            .map(|found| found.at.clone())
            .collect::<Vec<String>>()),
        Some(vec!["Photographs".to_owned()])
    );
    let root = manifest.as_ref().and_then(|manifest| {
        named(manifest, ROOT)
            .into_iter()
            .find(|item| item.name == ROOT)
    });
    assert!(
        root.and_then(|item| item.kept)
            .is_some_and(|why| why.contains("did not put there")),
        "the data location is still offered as one tree"
    );
    assert_eq!(
        manifest.map(|manifest| going(&manifest)),
        Some(vec![
            "/srv/media/downloads".to_owned(),
            "/srv/media/media/tv".to_owned()
        ]),
        "only the stack's own directories are offered"
    );
}

/// And the removal takes exactly those, never the tree they sit in.
#[tokio::test]
async fn a_removal_beside_the_operators_files_takes_only_the_stacks_own_directories() {
    let eraser = Erasing::willing();
    let ctx = a_machine()
        .surveying(Walking::holding(and_theirs()))
        .erasing(Arc::clone(&eraser) as Arc<dyn Eraser>);

    let removal = confirmed(&ctx, Tier::Media).await;

    assert!(removal.is_some_and(|removal| matches!(removal, Removal::Complete { .. })));
    assert_eq!(
        eraser.asked(),
        vec![
            PathBuf::from("/srv/media/downloads"),
            PathBuf::from("/srv/media/media/tv")
        ]
    );
}

/// A backup is taken before anything that cannot be made again goes, and the reading
/// says so — only for the tier that destroys it.
#[tokio::test]
async fn a_backup_is_taken_before_configuration_is_destroyed() {
    let vault = Arc::new(crate::app::fixtures::FakeArchive::roomy());
    let ctx = crate::app::fixtures::keeping(
        running(Lifecycle::Running, Health::Healthy)
            .with_images(Pulled::holding(vec![Pulled::image(
                SONARR,
                400,
                &["lemonfiber"],
            )]))
            .erasing(Erasing::willing()),
        &vault,
    );
    let said = read(&ctx, Tier::Configuration)
        .await
        .and_then(|manifest| manifest.backup);

    assert!(
        said.is_some_and(|said| said.contains("A backup is taken")),
        "the reading does not say a backup is taken before the configuration goes"
    );

    // And it is taken, rather than only said: the tier that destroys configuration
    // writes an archive before anything of it goes.
    let removal = confirmed(&ctx, Tier::Configuration).await;
    assert!(removal.is_some(), "the removal ran");
    assert_eq!(
        vault.written.lock().map(|written| written.len()).ok(),
        Some(1),
        "no archive was written before the configuration went"
    );
    assert_eq!(
        read(&ctx, Tier::Services)
            .await
            .map(|manifest| manifest.backup),
        Some(None)
    );
}

/// An image another project's container is standing on is listed, marked
/// with why it is kept, and left out of the total.
#[tokio::test]
async fn an_image_another_project_is_standing_on_is_listed_and_kept() {
    let ctx = a_machine().with_images(Pulled::holding(vec![Pulled::image(
        SONARR,
        400,
        &["lemonfiber", "someone-else"],
    )]));

    let manifest = read(&ctx, Tier::Services).await;
    let image = manifest
        .as_ref()
        .and_then(|manifest| named(manifest, SONARR).first().cloned());

    assert!(
        image
            .and_then(|item| item.kept)
            .is_some_and(|why| why.contains("standing on it")),
        "{manifest:?}"
    );
    assert_eq!(
        manifest.map(|manifest| manifest.bytes),
        Some(0),
        "a kept image is not room this frees"
    );
}

/// And the removal never asks the engine to take it.
#[tokio::test]
async fn a_shared_image_is_never_handed_to_the_engine_to_remove() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = watching(&runner).with_images(Pulled::holding(vec![Pulled::image(
        SONARR,
        400,
        &["lemonfiber", "someone-else"],
    )]));

    let removal = confirmed(&ctx, Tier::Services).await;

    assert!(removal.is_some_and(|removal| matches!(removal, Removal::Complete { .. })));
    assert!(
        !runner.ran("rm"),
        "the engine was asked to remove an image another project is standing on"
    );
}

/// An image only this stack stands on is handed over, so the case above is
/// a decision rather than a removal that never happens.
#[tokio::test]
async fn an_image_only_this_stack_stands_on_is_handed_to_the_engine() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = watching(&runner).with_images(Pulled::holding(vec![Pulled::image(
        SONARR,
        400,
        &["lemonfiber"],
    )]));

    let removal = confirmed(&ctx, Tier::Services).await;

    assert!(removal.is_some_and(|removal| matches!(removal, Removal::Complete { .. })));
    assert!(
        runner
            .seen()
            .iter()
            .any(|argv| argv.iter().any(|word| word == SONARR)),
        "{:?}",
        runner.seen()
    );
}

/// A machine whose programs are recorded, so what was run can be asserted on.
fn watching(runner: &Arc<Recording>) -> Ctx {
    a_context()
        .settings(settings())
        .engine(Arc::new(Reporting::holding(
            &["sonarr"],
            Lifecycle::Exited,
            Health::None,
        )))
        .runner(Arc::clone(runner) as Arc<dyn Runner>)
        .build()
        .with_filesystem(a_filesystem())
        .with_images(Pulled::holding(Vec::new()))
        .surveying(Walking::holding(only_ours()))
        .erasing(Erasing::willing())
}

/// An engine that refuses to remove an image keeps its own words,
/// and the command that finishes it is handed back.
#[tokio::test]
async fn an_image_the_engine_would_not_remove_is_named_with_what_it_said() {
    let runner = Arc::new(Recording::answering(Ok(
        lemonfiber_fixtures::support::refused("image is in use by a container"),
    )));
    let ctx = watching(&runner).with_images(Pulled::holding(vec![Pulled::image(
        SONARR,
        400,
        &["lemonfiber"],
    )]));

    let removal = confirmed(&ctx, Tier::Services).await;
    let stuck = removal.as_ref().map(left).unwrap_or_default();

    assert!(
        stuck
            .iter()
            .any(|one| one.name == SONARR && one.why.contains("in use")),
        "{stuck:?}"
    );
    assert!(
        stuck
            .iter()
            .any(|one| one.by_hand == format!("docker image rm {SONARR}")),
        "{stuck:?}"
    );
}

/// A stop Compose refused is reported rather than read as done.
///
/// A lifecycle command answers with a report whichever way the process went, so a
/// stop that failed and one that worked come back the same way — and a removal that
/// read the first as the second would carry on removing the files of a service that
/// is still running and still writing to them.
#[tokio::test]
async fn a_stop_that_compose_refused_is_reported_rather_than_read_as_done() {
    let runner = Arc::new(Recording::answering(Ok(
        lemonfiber_fixtures::support::refused("no configuration file provided"),
    )));
    let ctx = watching(&runner);

    let removal = confirmed(&ctx, Tier::Stop).await;
    let stuck = removal.as_ref().map(left).unwrap_or_default();

    assert_eq!(stuck.len(), 1, "{stuck:?}");
    assert!(
        stuck
            .iter()
            .any(|one| one.name.contains("running services") && one.why.contains("did not succeed")),
        "{stuck:?}"
    );
    assert!(
        stuck
            .iter()
            .any(|one| one.by_hand == "docker compose --project-name lemonfiber stop"),
        "{stuck:?}"
    );
}

/// Nothing this runs, and nothing it hands back about its own files, asks
/// the operator to become somebody else.
#[tokio::test]
async fn nothing_it_runs_or_hands_back_about_its_own_files_escalates() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = kept(
        watching(&runner)
            .with_images(Pulled::holding(vec![Pulled::image(
                SONARR,
                400,
                &["lemonfiber"],
            )]))
            .erasing(Erasing::refusing("permission denied")),
    );

    let mut instructions = Vec::new();
    for tier in [Tier::Stop, Tier::Services, Tier::Configuration] {
        let removal = confirmed(&ctx, tier).await;
        instructions.extend(removal.as_ref().map(left).unwrap_or_default());
    }

    assert!(
        !instructions.is_empty(),
        "nothing was left behind, so no instruction was checked"
    );
    let escalating: Vec<&Left> = instructions
        .iter()
        .filter(|one| one.by_hand.contains("sudo") || one.by_hand.contains("runas"))
        .collect();
    assert!(escalating.is_empty(), "{escalating:?}");

    let ran = runner.seen();
    assert!(!ran.is_empty(), "nothing was run, so nothing was checked");
    let escalated: Vec<&Vec<String>> = ran
        .iter()
        .filter(|argv| argv.iter().any(|word| word == "sudo" || word == "runas"))
        .collect();
    assert!(escalated.is_empty(), "{escalated:?}");
}

/// What is still coming down is reported before anything stops.
#[tokio::test]
async fn what_is_still_coming_down_is_reported_before_anything_stops() {
    let ctx = a_context()
        .settings(Settings {
            protocols: crate::config::Protocols::both(),
            ..settings()
        })
        .engine(Arc::new(Reporting::holding(
            &["sabnzbd"],
            Lifecycle::Running,
            Health::Healthy,
        )))
        .build()
        .with_filesystem(Arc::new(
            SeedFs::keyed(None, Some(lemonfiber_fixtures::downloads::SAB_KEY_INI)).with_facts(
                StorageFacts {
                    point: PathBuf::from(ROOT),
                    kind: FsKind::Linking("apfs".to_owned()),
                    removable: false,
                    available: 100,
                    total: 1_000,
                },
            ),
        ))
        .with_http(lemonfiber_fixtures::downloads::downloads(
            lemonfiber_fixtures::downloads::QBIT_TORRENTS,
            lemonfiber_fixtures::downloads::SAB_QUEUE,
        ))
        .with_images(Pulled::holding(Vec::new()))
        .surveying(Walking::holding(only_ours()))
        .erasing(Erasing::willing());

    let manifest = read(&ctx, Tier::Services).await;

    assert!(
        manifest
            .as_ref()
            .is_some_and(|manifest| manifest.coming.iter().any(|one| one.name == "Linux.nzb")),
        "{manifest:?}"
    );
}

/// A reading touches nothing, whatever it found.
#[tokio::test]
async fn a_reading_removes_nothing() {
    let eraser = Erasing::willing();
    let ctx = a_machine().erasing(Arc::clone(&eraser) as Arc<dyn Eraser>);

    let answered = answer(&ctx, Removing::surveying(Tier::Configuration)).await;

    assert_eq!(
        answered.map(|answered| answered.removal),
        Some(Removal::Surveyed)
    );
    assert_eq!(eraser.asked(), Vec::<PathBuf>::new());
}

/// The dispatcher routes it, which is the half a test of the handler alone does not
/// cover: this file is compiled with the crate, and the arm is in the copy that is.
#[tokio::test]
async fn the_dispatcher_routes_a_removal_to_the_reading() {
    let outcome = crate::app::dispatch(
        crate::app::Command::Uninstall(Removing::surveying(Tier::Stop)),
        &a_machine(),
    )
    .await;

    assert!(
        matches!(outcome, Ok(Outcome::Uninstall(_))),
        "the dispatcher did not answer with a removal"
    );
}

/// And a rehearsal ends where a confirmed run begins: the agreement was taken and
/// nothing was touched.
#[tokio::test]
async fn a_rehearsal_takes_the_agreement_and_touches_nothing() {
    let eraser = Erasing::willing();
    let ctx = a_machine()
        .erasing(Arc::clone(&eraser) as Arc<dyn Eraser>)
        .rehearsing();

    let removal = confirmed(&ctx, Tier::Media).await;

    assert_eq!(removal, Some(Removal::Confirmed));
    assert_eq!(eraser.asked(), Vec::<PathBuf>::new());
}
