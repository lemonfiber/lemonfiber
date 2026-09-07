//! Driving a removal against a machine a test wrote down.
//!
//! Every case here reaches the real survey and the real removal. The engine, the
//! image listing, the walk and the eraser are the seams; nothing else is stood in
//! for, so what these assert is what an operator would get.
//!
//! A run that refused answers with nothing rather than with an empty manifest, and
//! every assertion is made on that `Option` — so a case whose run refused fails on
//! the assertion it meant to make instead of on a shape it had to unwrap.

use std::path::PathBuf;
use std::sync::Arc;

use lemonfiber_fixtures::erasing::Erasing;
use lemonfiber_fixtures::pulled::Pulled;
use lemonfiber_fixtures::support::{spoke, Recording, Reporting, SeedFs};
use lemonfiber_fixtures::walking::Walking;
use lemonfiber_ports::docker::{Health, Lifecycle};
use lemonfiber_ports::occupancy::Occupant;

use super::uninstalled;
use crate::app::{Ctx, Outcome, Removing};
use crate::config::Settings;
use crate::ports::filesystem::{Eraser, FsKind, StorageFacts};
use crate::ports::Runner;
use crate::test_support::{a_context, nowhere};
use crate::uninstall::{
    Item, Left, Manifest, Removal, Sort, Tier, ANOTHER_READING, NEEDS_AGREEING,
};

/// The data location every case here is about.
const ROOT: &str = "/srv/media";

/// One image this stack declares, named once so a case does not spell the tag twice.
const SONARR: &str = "lscr.io/linuxserver/sonarr:4.0.15";

/// Settings naming a project, a layout and a data location — the three a survey
/// reads before it asks anything.
fn settings() -> Settings {
    Settings {
        project: "lemonfiber".to_owned(),
        env_file: Some(PathBuf::from("/cfg/lemonfiber/.env")),
        stack_dir: Some(PathBuf::from("/data/lemonfiber/stack")),
        data_root: Some(PathBuf::from(ROOT)),
        ..Settings::default()
    }
}

/// A file the walk reports at a path beneath the data location.
fn file(path: &str, bytes: u64) -> Occupant {
    Occupant {
        path: PathBuf::from(path),
        bytes,
        identity: None,
    }
}

/// A tree holding only what the stack itself writes.
fn only_ours() -> Vec<Occupant> {
    vec![
        file("/srv/media/downloads/A.Show/a.mkv", 1_000),
        file("/srv/media/media/tv/A Show/S01E01.mkv", 2_000),
    ]
}

/// The same tree, with something of the operator's beside it.
fn and_theirs() -> Vec<Occupant> {
    let mut tree = only_ours();
    tree.push(file("/srv/media/Photographs/2019/a.jpg", 500));
    tree
}

/// A filesystem answering every location, over an ordinary local disk.
fn a_filesystem() -> Arc<SeedFs> {
    over(FsKind::Linking("apfs".to_owned()), false)
}

/// A filesystem reporting the data location on a given sort of volume.
fn over(kind: FsKind, removable: bool) -> Arc<SeedFs> {
    Arc::new(SeedFs::keyed(None, None).with_facts(StorageFacts {
        point: PathBuf::from(ROOT),
        kind,
        removable,
        available: 100,
        total: 1_000,
    }))
}

/// A machine with a stack the engine is holding, one image pulled for it, and
/// nothing but the stack's own files on the disk.
fn a_machine() -> Ctx {
    running(Lifecycle::Running, Health::Healthy)
        .with_images(Pulled::holding(vec![Pulled::image(
            SONARR,
            400,
            &["lemonfiber"],
        )]))
        .erasing(Erasing::willing())
}

/// The same machine, with its services in a given state.
fn running(lifecycle: Lifecycle, health: Health) -> Ctx {
    a_context()
        .settings(settings())
        .engine(Arc::new(Reporting::holding(&["sonarr"], lifecycle, health)))
        .build()
        .with_filesystem(a_filesystem())
        .with_images(Pulled::holding(Vec::new()))
        .surveying(Walking::holding(only_ours()))
        .erasing(Erasing::willing())
}

/// A machine whose container engine is not there at all.
fn no_engine() -> Ctx {
    a_context()
        .settings(settings())
        .engine(Arc::new(Reporting::absent()))
        .build()
        .with_filesystem(a_filesystem())
        .with_images(Pulled::unreachable("no daemon here"))
        .surveying(Walking::holding(only_ours()))
        .erasing(Erasing::willing())
}

/// What a run came to, or nothing where it refused.
async fn answer(ctx: &Ctx, asked: Removing) -> Option<crate::uninstall::Uninstall> {
    match uninstalled(ctx, asked).await {
        Ok(Outcome::Uninstall(answered)) => Some(answered),
        Ok(_) | Err(_) => None,
    }
}

/// What a reading of one tier found, or nothing where it refused.
async fn read(ctx: &Ctx, tier: Tier) -> Option<Manifest> {
    answer(ctx, Removing::surveying(tier))
        .await
        .map(|answered| answered.manifest)
}

/// What a confirmed run came to, answering the reading it printed where the tier
/// takes one.
async fn confirmed(ctx: &Ctx, tier: Tier) -> Option<Removal> {
    let agreement = read(ctx, tier).await.map(|manifest| manifest.agreement);
    let asked = Removing::surveying(tier)
        .confirmed(true)
        .agreeing(agreement.filter(|_| tier.needs_its_own_agreement()));
    answer(ctx, asked).await.map(|answered| answered.removal)
}

/// The lines of a manifest whose name holds this text.
fn named(manifest: &Manifest, contains: &str) -> Vec<Item> {
    manifest
        .items
        .iter()
        .filter(|item| item.name.contains(contains))
        .cloned()
        .collect()
}

/// The names of the lines a manifest says are going.
fn going(manifest: &Manifest) -> Vec<String> {
    manifest
        .items
        .iter()
        .filter(|item| item.goes())
        .map(|item| item.name.clone())
        .collect()
}

/// What a removal could not take, or an empty list where it took everything.
fn left(removal: &Removal) -> Vec<Left> {
    match removal {
        Removal::Partial { left, .. } => left.clone(),
        Removal::Surveyed | Removal::Confirmed | Removal::Complete { .. } => Vec::new(),
    }
}

/// The credentials a removal reported destroying.
fn credentials(removal: &Removal) -> Vec<String> {
    match removal {
        Removal::Complete { credentials, .. } | Removal::Partial { credentials, .. } => {
            credentials.clone()
        }
        Removal::Surveyed | Removal::Confirmed => Vec::new(),
    }
}

// --- The manifest ------------------------------------------------------------

/// The list, not a count — the container the engine is holding, the network
/// it was on, the image pulled for it, and what that image occupies.
#[tokio::test]
async fn a_manifest_names_the_containers_the_network_and_the_images_with_a_total() {
    let manifest = read(&a_machine(), Tier::Services).await;

    assert!(
        manifest
            .as_ref()
            .is_some_and(|manifest| manifest.items.len() > 2),
        "a manifest of this many lines is a summary: {manifest:?}"
    );
    assert_eq!(
        manifest
            .as_ref()
            .map(|manifest| named(manifest, "lemonfiber-sonarr").len()),
        Some(1),
        "the container the engine is holding is named"
    );
    assert_eq!(
        manifest
            .as_ref()
            .and_then(|manifest| named(manifest, "lemonfiber_default")
                .first()
                .map(|item| item.sort)),
        Some(Sort::Network)
    );
    let image = manifest
        .as_ref()
        .and_then(|manifest| named(manifest, SONARR).first().cloned());
    assert_eq!(image.as_ref().map(|item| item.sort), Some(Sort::Image));
    assert_eq!(image.and_then(|item| item.bytes), Some(400));
    assert_eq!(
        manifest.map(|manifest| manifest.bytes),
        Some(400),
        "the total is what the lines that go occupy"
    );
}

/// An image the stack declares and this machine never pulled is not on the
/// list, because there is nothing there to remove.
#[tokio::test]
async fn an_image_this_machine_never_pulled_is_not_listed() {
    let manifest = read(
        &running(Lifecycle::Running, Health::Healthy),
        Tier::Services,
    )
    .await;

    assert_eq!(
        manifest.map(|manifest| named(&manifest, "lscr.io/linuxserver").len()),
        Some(0)
    );
}

/// The tier that removes nothing still shows the list, so "nothing
/// is removed" is checkable rather than something to take on trust.
#[tokio::test]
async fn the_tier_that_removes_nothing_still_lists_what_it_leaves() {
    let manifest = read(&a_machine(), Tier::Stop).await;

    assert_eq!(manifest.as_ref().map(|manifest| manifest.bytes), Some(0));
    assert_eq!(manifest.as_ref().map(going), Some(Vec::new()));
    assert!(
        manifest.is_some_and(|manifest| !manifest.items.is_empty()),
        "it lists nothing at all, so there was nothing to check"
    );
}

// --- The library -------------------------------------------------------------

/// No tier below the media one reaches anything under the data
/// location, and each says the library survives.
#[tokio::test]
async fn no_tier_below_the_media_one_reaches_the_data_location() {
    let ctx = a_machine();
    for tier in [Tier::Stop, Tier::Services, Tier::Configuration] {
        let manifest = read(&ctx, tier).await;
        let reaching = manifest.as_ref().map(|manifest| {
            manifest
                .items
                .iter()
                .filter(|item| item.name.starts_with(ROOT))
                .count()
        });

        assert_eq!(reaching, Some(0), "{tier:?} reaches the library");
        assert!(
            manifest.is_some_and(|manifest| manifest.keeps.to_lowercase().contains("library")),
            "{tier:?} does not promise the library survives"
        );
    }
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

// --- Files the stack did not write -------------------------------------------

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

/// A data location on a network share is said, and saying it makes the reading a
/// different one — which is the additional confirmation that case asks for.
#[tokio::test]
async fn a_data_location_on_a_network_share_is_said_and_changes_the_reading() {
    let across = read(
        &a_machine().with_filesystem(over(FsKind::classify("smbfs"), false)),
        Tier::Media,
    )
    .await;
    let local = read(&a_machine(), Tier::Media).await;

    assert!(
        across
            .as_ref()
            .and_then(|manifest| manifest.volume.clone())
            .is_some_and(|said| said.contains("network share")),
        "{across:?}"
    );
    assert_eq!(
        local.as_ref().and_then(|manifest| manifest.volume.clone()),
        None
    );
    assert_ne!(
        across.map(|manifest| manifest.agreement),
        local.map(|manifest| manifest.agreement)
    );
}

/// And so is a drive that unplugs, for the same reason.
#[tokio::test]
async fn a_data_location_on_a_drive_that_unplugs_is_said() {
    let manifest = read(
        &a_machine().with_filesystem(over(FsKind::Linking("exfat".to_owned()), true)),
        Tier::Media,
    )
    .await;

    assert!(
        manifest
            .and_then(|manifest| manifest.volume)
            .is_some_and(|said| said.contains("unplugs")),
        "a removable drive is not said"
    );
}

// --- Configuration and credentials -------------------------------------------

/// The tier that removes configuration destroys the credentials, and says
/// which ones it destroyed.
#[tokio::test]
async fn removing_configuration_destroys_the_credentials_and_names_them() {
    let eraser = Erasing::willing();
    let ctx = a_machine().erasing(Arc::clone(&eraser) as Arc<dyn Eraser>);

    let removal = confirmed(&ctx, Tier::Configuration).await;
    let named = removal.as_ref().map(credentials).unwrap_or_default();

    assert!(
        named.iter().any(|at| at.rsplit('/').next() == Some(".env")),
        "the settings file holds the VPN key and the provider password: {named:?}"
    );
    assert!(named.iter().any(|at| at.contains("admission")), "{named:?}");
    assert_eq!(
        eraser.asked(),
        vec![
            PathBuf::from("/cfg/lemonfiber"),
            PathBuf::from("/data/lemonfiber")
        ],
        "the two directories everything sits under, and nothing inside them twice"
    );
}

/// And a tier that destroys no credential says so by naming none.
#[tokio::test]
async fn a_removal_that_took_no_credential_names_none() {
    let removal = confirmed(&a_machine(), Tier::Services).await;

    assert_eq!(removal.as_ref().map(credentials), Some(Vec::new()));
}

/// A backup is offered before anything that cannot be made again goes, and
/// only by the tier that destroys it.
#[tokio::test]
async fn a_backup_is_offered_before_configuration_is_destroyed() {
    let ctx = a_machine();
    let offered = read(&ctx, Tier::Configuration)
        .await
        .and_then(|manifest| manifest.backup);

    assert!(
        offered.is_some_and(|said| said.contains("lemonfiber backup")),
        "no backup was offered before the configuration goes"
    );
    assert_eq!(
        read(&ctx, Tier::Services)
            .await
            .map(|manifest| manifest.backup),
        Some(None)
    );
}

// --- What lemonfiber cannot remove -------------------------------------------

/// Every reading lists what an uninstall leaves behind, with how to remove
/// each of them on this machine.
#[tokio::test]
async fn every_reading_lists_what_it_cannot_remove_with_a_way_to_remove_it() {
    let manifest = read(&a_machine(), Tier::Services).await;
    let outside = manifest
        .map(|manifest| manifest.outside)
        .unwrap_or_default();

    assert!(outside.len() >= 4, "{outside:?}");
    let silent: Vec<&str> = outside
        .iter()
        .filter(|entry| entry.by_hand.split_whitespace().count() < 4)
        .map(|entry| entry.what.as_str())
        .collect();
    assert!(
        silent.is_empty(),
        "these are listed with no way off: {silent:?}"
    );
    assert!(
        outside.iter().any(|entry| entry.found),
        "nothing was looked for: {outside:?}"
    );
    assert!(
        outside.iter().any(|entry| !entry.found),
        "everything reads as found, which means nothing was looked for either"
    );
}

// --- A machine that is broken ------------------------------------------------

/// An engine that will not answer does not stop a reading. The
/// containers are listed from what the stack declares, kept with the reason, and
/// the gap is stated in the engine's own words.
#[tokio::test]
async fn an_unreachable_engine_still_produces_a_manifest_and_says_what_it_could_not_read() {
    let manifest = read(&no_engine(), Tier::Services).await;

    assert_eq!(
        manifest
            .as_ref()
            .map(|manifest| manifest.confidence.complete),
        Some(false)
    );
    assert!(
        manifest.as_ref().is_some_and(|manifest| manifest
            .confidence
            .unread
            .iter()
            .any(|why| why.contains("no daemon here"))),
        "{manifest:?}"
    );
    assert!(
        manifest
            .as_ref()
            .and_then(|manifest| named(manifest, "lemonfiber-sonarr").first().cloned())
            .is_some_and(|item| !item.goes()),
        "a container an unreachable engine is still holding is listed and kept"
    );
    assert_eq!(manifest.map(|manifest| manifest.bytes), Some(0));
}

/// With the engine down, a confirmed removal still runs, reports
/// what it could not do, and hands back the command that finishes it.
#[tokio::test]
async fn a_removal_the_engine_refused_is_reported_with_the_command_that_finishes_it() {
    let removal = confirmed(&no_engine(), Tier::Services).await;
    let stuck = removal.as_ref().map(left).unwrap_or_default();

    assert_eq!(stuck.len(), 1, "{stuck:?}");
    assert!(
        stuck.first().is_some_and(|left| left
            .by_hand
            .contains("docker compose --project-name lemonfiber down")),
        "{stuck:?}"
    );
}

/// A stack description that will not read is a gap in the manifest rather
/// than a refusal, so a machine whose stack is broken can still be left.
#[tokio::test]
async fn a_stack_description_that_cannot_be_read_still_produces_a_manifest() {
    let ctx = a_context()
        .over(nowhere())
        .settings(settings())
        .build()
        .with_filesystem(a_filesystem())
        .with_images(Pulled::holding(Vec::new()))
        .surveying(Walking::holding(only_ours()))
        .erasing(Erasing::willing());

    let manifest = read(&ctx, Tier::Configuration).await;

    assert_eq!(
        manifest
            .as_ref()
            .map(|manifest| manifest.confidence.complete),
        Some(false)
    );
    assert!(
        manifest.is_some_and(|manifest| !manifest.items.is_empty()),
        "the configuration is still enumerated from the layout"
    );
}

/// A run that cannot say where lemonfiber keeps its own files names the
/// usual places, removes none of them, and says why.
#[tokio::test]
async fn a_run_that_cannot_say_where_its_files_go_names_the_usual_places_and_takes_none() {
    let eraser = Erasing::willing();
    let ctx = a_context()
        .settings(Settings {
            project: "lemonfiber".to_owned(),
            data_root: Some(PathBuf::from(ROOT)),
            ..Settings::default()
        })
        .build()
        .with_filesystem(a_filesystem())
        .with_images(Pulled::holding(Vec::new()))
        .surveying(Walking::holding(only_ours()))
        .erasing(Arc::clone(&eraser) as Arc<dyn Eraser>);

    let answered = answer(
        &ctx,
        Removing::surveying(Tier::Configuration).confirmed(true),
    )
    .await
    .map(|answered| answered.manifest);

    assert_eq!(
        answered
            .as_ref()
            .map(|manifest| manifest.confidence.complete),
        Some(false)
    );
    assert_eq!(
        answered.as_ref().map(going),
        Some(Vec::new()),
        "a location this run had to fall back to is never taken"
    );
    assert!(
        answered.is_some_and(|manifest| manifest
            .items
            .iter()
            .all(|item| item.name.starts_with("~/"))),
        "the usual places are not named"
    );
    assert_eq!(eraser.asked(), Vec::<PathBuf>::new());
}

/// A path the platform refuses is named with what the platform said and the
/// way to finish it.
#[tokio::test]
async fn a_path_the_platform_refused_is_named_with_what_it_said_and_how_to_finish_it() {
    let ctx = a_machine().erasing(Erasing::refusing("permission denied"));

    let removal = confirmed(&ctx, Tier::Configuration).await;
    let stuck = removal.as_ref().map(left).unwrap_or_default();

    assert_eq!(stuck.len(), 2, "{stuck:?}");
    let unhelpful: Vec<&Left> = stuck
        .iter()
        .filter(|one| {
            one.why != "permission denied"
                || !one.by_hand.contains(&one.name)
                || !one.by_hand.contains("owns it")
        })
        .collect();
    assert!(unhelpful.is_empty(), "{unhelpful:?}");
}

/// A tree that is there and will not be read is a gap in the reading rather than a
/// refusal, and the gap names the directory it is about.
#[tokio::test]
async fn a_directory_that_will_not_be_walked_is_named_and_the_reading_says_so() {
    let ctx = a_machine().surveying(Walking::refusing("permission denied"));

    let manifest = read(&ctx, Tier::Configuration).await;
    let unread = manifest
        .as_ref()
        .map(|manifest| manifest.confidence.unread.clone())
        .unwrap_or_default();

    assert!(
        unread.iter().any(|why| why.contains("/cfg/lemonfiber")),
        "{unread:?}"
    );
    assert!(
        unread.iter().any(|why| why.contains("permission denied")),
        "{unread:?}"
    );
    assert_eq!(
        manifest.map(|manifest| manifest.confidence.complete),
        Some(false)
    );
}

/// And so is a data location that will not be walked — which is a different gap in
/// different words, because what it costs is different: no size, and no way to tell
/// what beneath it is not the stack's.
#[tokio::test]
async fn a_data_location_that_will_not_be_walked_says_what_that_costs() {
    let ctx = a_machine().surveying(Walking::refusing("permission denied"));

    let manifest = read(&ctx, Tier::Media).await;

    assert!(
        manifest
            .map(|manifest| manifest.confidence.unread)
            .unwrap_or_default()
            .iter()
            .any(|why| why.contains("the data location is there")),
        "a data location that would not be read says nothing about it"
    );
}

/// A machine that never chose a data location has nothing beneath one to report,
/// which is a reading rather than a gap: it has not filled anything yet.
#[tokio::test]
async fn a_machine_with_no_data_location_has_nothing_of_the_operators_to_list() {
    let ctx = a_context()
        .settings(Settings {
            project: "lemonfiber".to_owned(),
            env_file: Some(PathBuf::from("/cfg/lemonfiber/.env")),
            stack_dir: Some(PathBuf::from("/data/lemonfiber/stack")),
            ..Settings::default()
        })
        .build()
        .with_filesystem(a_filesystem())
        .with_images(Pulled::holding(Vec::new()))
        .surveying(Walking::holding(only_ours()))
        .erasing(Erasing::willing());

    let manifest = read(&ctx, Tier::Media).await;

    assert_eq!(
        manifest.as_ref().map(|manifest| manifest.items.len()),
        Some(0)
    );
    assert_eq!(
        manifest.map(|manifest| manifest.confidence.complete),
        Some(true),
        "a location nobody chose is not a reading that failed"
    );
}

/// A program that is not on this machine at all keeps its own words, which is a
/// different answer from one that ran and refused.
#[tokio::test]
async fn a_program_that_is_not_installed_is_reported_in_its_own_words() {
    let ctx = a_context()
        .settings(settings())
        .engine(Arc::new(Reporting::holding(
            &["sonarr"],
            Lifecycle::Exited,
            Health::None,
        )))
        .runner(Arc::new(lemonfiber_fixtures::support::Scripted(Err(
            crate::ports::process::Failure::NotFound {
                program: "docker".to_owned(),
            },
        ))))
        .build()
        .with_filesystem(a_filesystem())
        .with_images(Pulled::holding(vec![Pulled::image(
            SONARR,
            400,
            &["lemonfiber"],
        )]))
        .surveying(Walking::holding(only_ours()))
        .erasing(Erasing::willing());

    let removal = confirmed(&ctx, Tier::Services).await;
    let stuck = removal.as_ref().map(left).unwrap_or_default();

    assert!(
        stuck
            .iter()
            .any(|one| one.name == SONARR && one.why.contains("docker")),
        "{stuck:?}"
    );
}

// --- Images shared with other projects ---------------------------------------

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

// --- No privilege escalation --------------------------------------------------

/// Nothing this runs, and nothing it hands back about its own files, asks
/// the operator to become somebody else.
#[tokio::test]
async fn nothing_it_runs_or_hands_back_about_its_own_files_escalates() {
    let runner = Arc::new(Recording::answering(Ok(spoke(""))));
    let ctx = watching(&runner)
        .with_images(Pulled::holding(vec![Pulled::image(
            SONARR,
            400,
            &["lemonfiber"],
        )]))
        .erasing(Erasing::refusing("permission denied"));

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

// --- Downloads still coming down ----------------------------------------------

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

// --- Rehearsal and reading ----------------------------------------------------

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
