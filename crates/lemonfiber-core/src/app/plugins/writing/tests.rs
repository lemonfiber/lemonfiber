//! Writing what an install lands: the record of each path it brings into being, and a
//! write the machine refuses.

use std::path::PathBuf;

use super::{carry_out, missing_from};
use crate::app::Ctx;
use crate::plugin::{Lands, Write};

/// A machine whose configuration and stack directories are both known; answers where its
/// journal is.
fn machine(name: &str) -> (Ctx, PathBuf) {
    let mut ctx = crate::test_support::a_context().build();
    let env = crate::test_support::env_without_password(name);
    ctx.settings.stack_dir = Some(env.with_file_name("data").join("stack"));
    ctx.settings.env_file = Some(env);
    let journal = crate::app::targets::layout(&ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    (ctx, journal)
}

#[test]
fn every_missing_directory_above_a_file_is_recorded_and_a_file_already_there_is_not() {
    let dir = lemonfiber_fixtures::scratch::Scratch::new("missing-from");
    let file = dir.join("a").join("b").join("kept.env");

    assert_eq!(
        missing_from(&file, false),
        vec![dir.join("a"), dir.join("a").join("b"), file.clone()]
    );
    assert_eq!(
        missing_from(&dir.join("x").join("y"), true),
        vec![dir.join("x"), dir.join("x").join("y")]
    );
    let _ = std::fs::create_dir_all(dir.join("a").join("b"));
    let _ = std::fs::write(&file, "held");
    assert!(missing_from(&file, false).is_empty());
}

#[test]
fn a_project_whose_journal_cannot_be_written_is_not_written() {
    let (ctx, journal) = machine("unjournalled-document");
    let _ = std::fs::create_dir_all(&journal);
    let project = ctx
        .settings
        .stack_dir
        .clone()
        .unwrap_or_default()
        .join(crate::project::FILE);

    let written = carry_out(
        &ctx,
        "tools",
        "stamp",
        &[Write {
            path: project.clone(),
            lands: Lands::Document {
                previous: None,
                text: "name: lemonfiber\n".to_owned(),
            },
        }],
    );

    assert!(written.is_err());
    assert!(!project.exists());
}

#[test]
fn a_secrets_file_that_cannot_be_written_is_refused_naming_it() {
    let (ctx, journal) = machine("unwritten-secrets");
    let blocked = journal.with_file_name("blocked");
    let _ = std::fs::write(&blocked, "a file, not a directory");
    let path = blocked.join("tools.env");

    let refused = carry_out(
        &ctx,
        "tools",
        "stamp",
        &[Write {
            path: path.clone(),
            lands: Lands::Secrets(Vec::new()),
        }],
    )
    .err();

    assert!(
        refused.is_some_and(|problem| problem.summary.contains(&path.display().to_string())),
        "named"
    );
}
