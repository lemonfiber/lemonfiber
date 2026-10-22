use std::path::{Path, PathBuf};

use super::Pass;
use crate::app::Ctx;
use crate::journal::{Change, Kind};

#[test]
fn a_preview_is_not_a_write() {
    assert_ne!(Pass::Preview, Pass::Write);
}

/// A machine running the stack this build carries, whose media server kept its cache
/// inside its configuration directory; answers where the project is.
fn machine(name: &str) -> (Ctx, PathBuf) {
    let mut ctx = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .engine_volumes(lemonfiber_fixtures::volumed::Volumed::none())
        .build();
    let env = crate::test_support::env_without_password(name);
    ctx.settings.stack_dir = Some(env.with_file_name("data").join("stack"));
    ctx.settings.env_file = Some(env);
    let project = super::directory(&ctx).unwrap_or_default();
    let (from, _) = cache(&project);
    let _ = std::fs::create_dir_all(&from);
    let _ = std::fs::write(from.join("kept.db"), "state");
    let _ = std::fs::create_dir_all(project.join("config").join("caddy"));
    let _ = std::fs::write(
        project.join("config").join("caddy").join("Caddyfile"),
        "ours",
    );
    (ctx, project)
}

/// Move what the machine's stack keeps in its configuration tree into its mount tree.
fn moving(ctx: &Ctx) -> Result<(), Box<crate::error::Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|why| Box::new(crate::error::Diagnose::problem(&why)))?;
    super::moved(ctx, &manifest)
}

/// Every change the journal holds.
fn journal(ctx: &Ctx) -> Vec<Change> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| journal.changes().to_vec())
        .unwrap_or_default()
}

/// Where the media server's cache was kept, and where it is kept now.
fn cache(project: &Path) -> (PathBuf, PathBuf) {
    (
        project.join("config").join("jellyfin").join("cache"),
        project.join("mounts").join("jellyfin").join("cache"),
    )
}

#[test]
fn state_in_the_configuration_tree_is_moved_into_the_mount_tree_and_journalled() {
    let (ctx, project) = machine("moved");
    let (from, to) = cache(&project);
    assert!(from.is_dir());

    let moved = moving(&ctx);
    assert!(moved.is_ok(), "{moved:?}");

    assert!(!from.exists());
    assert!(to.join("kept.db").is_file());
    assert!(project.join("config/caddy/Caddyfile").is_file());
    assert!(!project.join("mounts/caddy").exists());
    assert!(journal(&ctx).iter().any(|change| matches!(&change.kind,
        Kind::Moved { from: was, to: now }
            if *was == from.display().to_string() && *now == to.display().to_string())));
}

#[test]
fn state_already_in_the_mount_tree_is_not_moved_over() {
    let (ctx, project) = machine("already");
    let (from, to) = cache(&project);
    let _ = std::fs::create_dir_all(&to);
    let _ = std::fs::write(to.join("theirs"), "theirs");

    assert!(moving(&ctx).is_ok());

    assert!(from.is_dir());
    assert!(to.join("theirs").is_file());
    assert!(!journal(&ctx)
        .iter()
        .any(|change| change.target == from.display().to_string()));
}

#[test]
fn nothing_is_moved_in_a_stack_directory_the_operator_named() {
    let (mut ctx, project) = machine("external");
    ctx.stack = crate::test_support::stack();
    let ours = super::directory(&ctx).unwrap_or_default();
    let (from, _) = cache(&ours);
    let _ = std::fs::create_dir_all(&from);

    assert!(moving(&ctx).is_ok());

    assert!(from.is_dir());
    assert!(!ours.join("mounts").exists());
    assert!(cache(&project).0.is_dir());
}

#[test]
fn a_rehearsal_moves_nothing() {
    let (mut ctx, project) = machine("rehearsed");
    ctx.dry_run = true;
    let (from, _) = cache(&project);

    assert!(moving(&ctx).is_ok());

    assert!(from.is_dir());
    assert!(journal(&ctx).is_empty());
}

#[test]
fn nothing_is_moved_where_the_move_cannot_be_journalled() {
    let (ctx, project) = machine("unjournalled-move");
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    let _ = std::fs::create_dir_all(&journal);
    let (from, to) = cache(&project);

    assert!(moving(&ctx).is_err());

    assert!(from.is_dir());
    assert!(!to.exists());
}

#[cfg(unix)]
mod links {
    use std::os::unix::fs::symlink;

    use super::*;

    #[test]
    fn a_link_where_the_state_was_is_neither_moved_nor_journalled() {
        let (ctx, project) = machine("linked");
        let (from, to) = cache(&project);
        let elsewhere = project.with_file_name("elsewhere");
        let _ = std::fs::create_dir_all(&elsewhere);
        let _ = std::fs::remove_dir_all(&from);
        let _ = symlink(&elsewhere, &from);

        assert!(moving(&ctx).is_ok());

        assert!(std::fs::symlink_metadata(&from).is_ok_and(|found| found.is_symlink()));
        assert!(std::fs::symlink_metadata(&to).is_err());
        assert!(!journal(&ctx)
            .iter()
            .any(|change| change.target == from.display().to_string()));
    }

    #[test]
    fn a_link_above_where_the_state_goes_is_neither_followed_nor_journalled() {
        let (ctx, project) = machine("linked-above");
        let (from, _) = cache(&project);
        let elsewhere = project.with_file_name("elsewhere-above");
        let _ = std::fs::create_dir_all(&elsewhere);
        let _ = std::fs::create_dir_all(project.join("mounts"));
        let _ = symlink(&elsewhere, project.join("mounts").join("jellyfin"));

        assert!(moving(&ctx).is_ok());

        assert!(from.is_dir());
        assert!(std::fs::read_dir(&elsewhere).is_ok_and(|entries| entries.count() == 0));
        assert!(!journal(&ctx)
            .iter()
            .any(|change| change.target == from.display().to_string()));
    }
}

mod copying;
mod profiled;
mod refused;
mod retiring;
mod rooted;
mod secrets;
