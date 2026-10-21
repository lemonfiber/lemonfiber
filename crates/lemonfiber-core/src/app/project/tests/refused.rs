//! A write of the project, of a secrets file or of the journal that the machine refuses
//! stops what was writing it.

use std::path::Path;

use lemonfiber_manifest::Manifest;

use super::machine;
use crate::app::Ctx;

/// The manifest of the stack this build carries.
fn manifest() -> Option<Manifest> {
    Manifest::from_toml(lemonfiber_bundled::STACK).ok()
}

/// A directory standing where the file `at` goes.
fn blocked(at: &Path) {
    let _ = std::fs::remove_file(at);
    let _ = std::fs::create_dir_all(at);
}

/// A file standing where the directory of the stack's secrets files goes.
fn secrets_blocked(ctx: &Ctx) {
    let file = manifest()
        .map(|manifest| crate::settings::Resolved::of(ctx, Some(&manifest), &[]))
        .and_then(|resolved| resolved.secrets_file("gluetun"))
        .unwrap_or_default();
    if let Some(directory) = file.parent() {
        let _ = std::fs::remove_dir_all(directory);
        let _ = std::fs::write(directory, "a file, not a directory");
    }
}

/// Prepare the project for a start, as a start does.
fn preparing(ctx: &Ctx) -> bool {
    manifest().is_some_and(|manifest| super::super::prepared(ctx, &manifest, &[], false).is_ok())
}

/// Write the project as a change to what is installed does.
fn rewriting(ctx: &Ctx) -> bool {
    manifest().is_some_and(|manifest| {
        super::super::rewritten(ctx, &manifest, &[], "plugin tools", "stamp").is_ok()
    })
}

#[test]
fn a_start_whose_project_cannot_be_written_is_refused() {
    let (ctx, project) = machine("unwritable-start");
    blocked(&project.join(crate::project::FILE));

    assert!(!preparing(&ctx));
}

#[test]
fn a_start_whose_secrets_cannot_be_written_is_refused() {
    let (ctx, _) = machine("unfiled-start");
    secrets_blocked(&ctx);

    assert!(!preparing(&ctx));
}

#[test]
fn a_rewrite_with_nowhere_to_journal_it_is_refused() {
    let (mut ctx, _) = machine("nowhere-rewrite");
    ctx.settings.env_file = None;

    assert!(!rewriting(&ctx));
}

#[test]
fn a_rewrite_whose_journal_cannot_be_written_is_refused() {
    let (ctx, project) = machine("unjournalled-rewrite");
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    blocked(&journal);

    assert!(!rewriting(&ctx));
    assert!(!project.join(crate::project::FILE).exists());
}

#[test]
fn a_rewrite_whose_project_cannot_be_written_is_refused() {
    let (ctx, project) = machine("unwritable-rewrite");
    blocked(&project.join(crate::project::FILE));

    assert!(!rewriting(&ctx));
}

#[test]
fn a_rewrite_whose_secrets_cannot_be_written_is_refused() {
    let (ctx, _) = machine("unfiled-rewrite");
    secrets_blocked(&ctx);

    assert!(!rewriting(&ctx));
}
