//! A volume the engine keeps for a stack service's mount, copied once into the mount's
//! own directory before a start.

use std::sync::Arc;

use lemonfiber_fixtures::volumed::Volumed;

use super::{journal, machine};
use crate::journal::Kind;

/// The engine's name for the volume the media server's cache was kept in.
const VOLUME: &str = "lemonfiber_jellyfin_cache";

/// Copy what the machine's engine keeps, as a start does.
async fn copying(ctx: &crate::app::Ctx) -> Result<(), Box<crate::error::Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|why| Box::new(crate::error::Diagnose::problem(&why)))?;
    super::super::copies(ctx, &manifest).await.made(ctx).await
}

/// Where the cache is kept now.
fn cache(project: &std::path::Path) -> std::path::PathBuf {
    project.join("mounts").join("jellyfin").join("cache")
}

#[tokio::test]
async fn a_volume_is_copied_into_the_empty_mount_once_and_journalled() {
    let (mut ctx, project) = machine("copied");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));
    let volumes = Volumed::holding(&[(VOLUME, &[("transcodes/kept", "state")])]);
    ctx.seams.engine_volumes = volumes.clone();

    let copied = copying(&ctx).await;

    assert!(copied.is_ok(), "{copied:?}");
    assert_eq!(
        std::fs::read_to_string(cache(&project).join("transcodes/kept"))
            .ok()
            .as_deref(),
        Some("state")
    );
    assert!(volumes
        .handed_over()
        .iter()
        .any(|(name, image)| name == VOLUME && image.contains("jellyfin")));
    assert!(journal(&ctx).iter().any(|change| matches!(&change.kind,
        Kind::Copied { volume, to }
            if volume == VOLUME && *to == cache(&project).display().to_string())));

    assert!(copying(&ctx).await.is_ok());
    assert_eq!(
        volumes.handed_over().len(),
        1,
        "a mount holding something is not copied into"
    );
}

#[tokio::test]
async fn nothing_is_copied_without_a_volume_or_into_a_stack_directory_the_operator_named() {
    let (mut ctx, project) = machine("uncopied");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));
    assert!(copying(&ctx).await.is_ok());
    assert!(!cache(&project).exists());

    let volumes = Volumed::holding(&[(VOLUME, &[("kept", "state")])]);
    ctx.seams.engine_volumes = volumes.clone();
    ctx.stack = crate::test_support::stack();

    assert!(copying(&ctx).await.is_ok());
    assert!(volumes.handed_over().is_empty());
}

#[tokio::test]
async fn a_volume_that_cannot_be_copied_stops_the_start_naming_it() {
    let (mut ctx, project) = machine("refused-copy");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));
    ctx.seams.engine_volumes = Volumed::refusing(&[VOLUME]) as Arc<_>;

    let refused = copying(&ctx).await.err();

    assert!(
        refused.is_some_and(
            |problem| problem.code.as_str() == "STACK-12" && problem.summary.contains(VOLUME)
        ),
        "named and refused"
    );
}

#[tokio::test]
async fn an_engine_that_cannot_say_what_it_keeps_stops_the_start_before_an_empty_mount_is_used() {
    let (mut ctx, project) = machine("unasked-copy");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));
    ctx.seams.engine_volumes = Volumed::silent() as Arc<_>;

    let refused = copying(&ctx).await.err();

    assert!(
        refused.is_some_and(|problem| problem.code.as_str().starts_with("DOCKER-")),
        "the engine's own refusal"
    );
    assert!(journal(&ctx).is_empty(), "nothing journalled");
}

#[cfg(unix)]
#[tokio::test]
async fn nothing_is_copied_through_a_link_standing_where_the_mount_goes() {
    let (mut ctx, project) = machine("linked-copy");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));
    let elsewhere = project.with_file_name("elsewhere");
    let _ = std::fs::create_dir_all(&elsewhere);
    let _ = std::fs::create_dir_all(project.join("mounts").join("jellyfin"));
    assert!(std::os::unix::fs::symlink(&elsewhere, cache(&project)).is_ok());
    let volumes = Volumed::holding(&[(VOLUME, &[("kept", "state")])]);
    ctx.seams.engine_volumes = volumes.clone() as Arc<_>;

    assert!(copying(&ctx).await.is_ok());

    assert!(volumes.handed_over().is_empty());
    assert!(std::fs::read_dir(&elsewhere).is_ok_and(|entries| entries.count() == 0));
}
