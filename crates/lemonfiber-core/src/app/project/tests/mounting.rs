//! A stack service's further mount source, made as the operator before a start where
//! nothing stands there, so the engine never makes it as root.

use super::{journal, machine, manifest, plan};
use crate::app::Ctx;
use crate::journal::Kind;

/// Make the mount sources of the stack this build carries, as a start does.
fn making(ctx: &Ctx) -> Result<(), Box<crate::error::Problem>> {
    making_beside(ctx, &[])
}

/// Make the mount sources of the stack this build carries beside `installed`, as a start
/// does.
fn making_beside(
    ctx: &Ctx,
    installed: &[crate::plugin::Installed],
) -> Result<(), Box<crate::error::Problem>> {
    manifest().map_or(Ok(()), |manifest| {
        super::super::mounting::made(ctx, &manifest, installed)
    })
}

/// Prepare the project of the stack this build carries for a run, starting where
/// `starting`.
fn preparing(ctx: &Ctx, starting: bool) -> bool {
    manifest()
        .is_some_and(|manifest| super::super::prepared(ctx, &manifest, &plan(), starting).is_ok())
}

/// Where the media server's cache is mounted from.
fn cache(project: &std::path::Path) -> std::path::PathBuf {
    project.join("mounts").join("jellyfin").join("cache")
}

/// Every directory the journal holds as made.
fn made(ctx: &Ctx) -> Vec<String> {
    journal(ctx)
        .into_iter()
        .filter_map(|change| match change.kind {
            Kind::Made { path } => Some(path),
            _ => None,
        })
        .collect()
}

#[test]
fn a_mount_source_not_there_is_made_and_journalled_parent_first() {
    let (ctx, project) = machine("mount-made");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));

    assert!(making(&ctx).is_ok());

    assert!(cache(&project).is_dir());
    let made = self::made(&ctx);
    let tree = project.join("mounts").display().to_string();
    let at = |path: &std::path::Path| {
        made.iter()
            .position(|one| *one == path.display().to_string())
    };
    assert!(
        at(&project.join("mounts")) < at(&cache(&project)),
        "{made:?}"
    );
    assert!(at(&cache(&project)).is_some(), "{made:?}");
    assert!(made.iter().all(|path| path.starts_with(&tree)), "{made:?}");

    let before = made.len();
    assert!(making(&ctx).is_ok());
    assert_eq!(self::made(&ctx).len(), before, "nothing made twice");
}

#[test]
fn a_start_makes_each_mount_source_and_a_run_starting_nothing_makes_none() {
    let (ctx, project) = machine("mount-started");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));

    assert!(preparing(&ctx, false));
    let before = cache(&project).exists();
    assert!(preparing(&ctx, true));

    assert!(!before);
    assert!(cache(&project).is_dir());
}

#[test]
fn a_stack_directory_the_operator_named_gets_its_mount_sources_too() {
    let (mut ctx, _) = machine("mount-named");
    ctx.stack = crate::test_support::stack();
    let ours = super::super::directory(&ctx).unwrap_or_default();

    assert!(making(&ctx).is_ok());

    assert!(cache(&ours).is_dir());
}

#[test]
fn a_stack_service_moved_in_place_gets_no_mount_source_of_the_stack() {
    let (ctx, project) = machine("mount-moved");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));
    let mut moved = crate::test_support::a_placed("jellyfin", &[], None, None);
    moved.replaces = Some("jellyfin".to_owned());
    let installed = [crate::test_support::an_installed("jellyfin", vec![moved])];

    assert!(making_beside(&ctx, &installed).is_ok());

    assert!(!cache(&project).exists());
    assert!(!made(&ctx).contains(&cache(&project).display().to_string()));
}

#[test]
fn a_rehearsal_makes_nothing() {
    let (mut ctx, project) = machine("mount-rehearsed");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));
    ctx.dry_run = true;

    assert!(making(&ctx).is_ok());

    assert!(!cache(&project).exists());
    assert!(journal(&ctx).is_empty());
}

#[test]
fn nothing_is_made_where_it_cannot_be_journalled() {
    let (ctx, project) = machine("mount-unjournalled");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    let _ = std::fs::create_dir_all(&journal);

    assert!(making(&ctx).is_err());

    assert!(!cache(&project).exists());
}

#[cfg(unix)]
#[test]
fn a_link_standing_where_a_mount_source_goes_is_left_as_it_is() {
    let (ctx, project) = machine("mount-linked");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));
    let elsewhere = project.with_file_name("mount-elsewhere");
    let _ = std::fs::create_dir_all(&elsewhere);
    let _ = std::fs::create_dir_all(project.join("mounts").join("jellyfin"));
    assert!(std::os::unix::fs::symlink(&elsewhere, cache(&project)).is_ok());

    assert!(making(&ctx).is_ok());

    assert!(std::fs::read_dir(&elsewhere).is_ok_and(|mut held| held.next().is_none()));
    assert!(!made(&ctx).contains(&cache(&project).display().to_string()));
}

#[cfg(unix)]
#[test]
fn a_mount_source_that_cannot_be_made_stops_the_start_naming_it() {
    use std::os::unix::fs::PermissionsExt as _;

    let (ctx, project) = machine("mount-unmade");
    let _ = std::fs::remove_dir_all(project.join("config").join("jellyfin").join("cache"));
    let tree = project.join("mounts");
    let _ = std::fs::create_dir_all(&tree);
    let _ = std::fs::set_permissions(&tree, std::fs::Permissions::from_mode(0o555));

    let refused = making(&ctx).err();
    let _ = std::fs::set_permissions(&tree, std::fs::Permissions::from_mode(0o755));

    assert!(
        refused.is_some_and(|problem| problem.summary.contains("mounts")),
        "named"
    );
}
