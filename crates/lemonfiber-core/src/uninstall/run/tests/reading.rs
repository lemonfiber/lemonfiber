//! What an uninstall reads before it removes anything.

use super::*;

/// A machine whose container engine is not there at all.
fn no_engine() -> Ctx {
    a_context()
        .settings(settings())
        .engine(Arc::new(Reporting::absent()))
        .build()
        .with_filesystem(a_filesystem())
        .with_images(Pulled::unreachable("no daemon here"))
        .with_occupancy(Walking::holding(only_ours()))
        .with_eraser(Erasing::willing())
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

/// The tier that removes configuration destroys the credentials, and says
/// which ones it destroyed.
#[tokio::test]
async fn removing_configuration_destroys_the_credentials_and_names_them() {
    let eraser = Erasing::willing();
    let ctx = a_machine().with_eraser(Arc::clone(&eraser) as Arc<dyn Eraser>);

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
        .with_occupancy(Walking::holding(only_ours()))
        .with_eraser(Erasing::willing());

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
        .with_occupancy(Walking::holding(only_ours()))
        .with_eraser(Arc::clone(&eraser) as Arc<dyn Eraser>);

    // The reading, which is what names the usual places. A confirmed run on this
    // machine is refused below: with nowhere it can say its files are, there is
    // nowhere to put the backup a destructive tier takes first.
    let answered = answer(&ctx, Removing::surveying(Tier::Configuration))
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

    // And a confirmed run takes none of them, because the backup that comes first
    // has nowhere to go on a machine that cannot say where its own files are.
    let refused = answer(
        &ctx,
        Removing::surveying(Tier::Configuration).confirmed(true),
    )
    .await;

    assert!(refused.is_none(), "a confirmed run was not refused");
    assert_eq!(eraser.asked(), Vec::<PathBuf>::new());
}

/// A path the platform refuses is named with what the platform said and the
/// way to finish it.
#[tokio::test]
async fn a_path_the_platform_refused_is_named_with_what_it_said_and_how_to_finish_it() {
    let ctx = a_machine().with_eraser(Erasing::refusing("permission denied"));

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
    let ctx = a_machine().with_occupancy(Walking::refusing("permission denied"));

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
    let ctx = a_machine().with_occupancy(Walking::refusing("permission denied"));

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
        .with_occupancy(Walking::holding(only_ours()))
        .with_eraser(Erasing::willing());

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
        .with_occupancy(Walking::holding(only_ours()))
        .with_eraser(Erasing::willing());

    let removal = confirmed(&ctx, Tier::Services).await;
    let stuck = removal.as_ref().map(left).unwrap_or_default();

    assert!(
        stuck
            .iter()
            .any(|one| one.name == SONARR && one.why.contains("docker")),
        "{stuck:?}"
    );
}

/// What lemonfiber keeps is sized from a walk of the two directories it keeps it
/// in, not guessed at — so the figure an operator reads before agreeing is the one
/// on their disk.
#[tokio::test]
async fn what_lemonfiber_keeps_is_sized_from_a_walk_of_where_it_keeps_it() {
    let ctx = a_machine().with_occupancy(Walking::holding(vec![
        file("/cfg/lemonfiber/.env", 400),
        file("/data/lemonfiber/config/sonarr/config.xml", 1_600),
    ]));

    let manifest = read(&ctx, Tier::Configuration).await;
    let sized =
        |manifest: &Manifest, at: &str| named(manifest, at).first().and_then(|item| item.bytes);

    assert_eq!(
        manifest
            .as_ref()
            .and_then(|manifest| sized(manifest, "/cfg/lemonfiber")),
        Some(400)
    );
    assert_eq!(
        manifest
            .as_ref()
            .and_then(|manifest| sized(manifest, "/data/lemonfiber")),
        Some(1_600)
    );
    assert_eq!(manifest.map(|manifest| manifest.bytes), Some(2_000));
}

/// Where lemonfiber keeps its own files is the surface's answer where the surface
/// gave one, and what the settings imply only where it did not.
///
/// The two can disagree — a run told where to keep its archives has been told where
/// its layout is — and a removal that worked it out from the settings anyway would
/// name a directory nobody pointed it at.
#[tokio::test]
async fn the_layout_a_surface_resolved_is_the_one_a_removal_names() {
    let elsewhere = a_context()
        .settings(Settings {
            env_file: Some(PathBuf::from("/elsewhere/lemonfiber/.env")),
            stack_dir: Some(PathBuf::from("/elsewhere/lemonfiber/stack")),
            ..settings()
        })
        .build()
        .with_filesystem(a_filesystem())
        .with_images(Pulled::holding(Vec::new()))
        .with_occupancy(Walking::holding(only_ours()))
        .with_eraser(Erasing::willing());
    let vault = Arc::new(crate::app::fixtures::FakeArchive::roomy());
    let ctx = crate::app::fixtures::keeping(elsewhere, &vault);

    let manifest = read(&ctx, Tier::Configuration).await;

    assert_eq!(
        manifest
            .as_ref()
            .map(|manifest| named(manifest, "/cfg/lemonfiber/.env").len()),
        Some(1),
        "the layout it was handed is not the one it named"
    );
    assert_eq!(
        manifest.map(|manifest| named(&manifest, "/elsewhere").len()),
        Some(0),
        "it worked the layout out from the settings instead"
    );
}
