use std::path::{Path, PathBuf};

use super::unlinked;
use crate::app::Ctx;
use crate::error::codes::life::MOUNT_LINKED;
use crate::project::FILE;

const PROJECT: &str = "services:\n  shelf:\n    volumes:\n      - library:/data\n      - ./config/shelf:/config\n      - ./mounts/shelf/cache:/cache:ro\n  reader:\n    volumes:\n      - ./config/reader:/config\n";

/// A machine whose project directory holds `text` as its project, where there is one;
/// answers where the project directory is.
fn machine(name: &str, text: Option<&str>) -> (Ctx, PathBuf) {
    let mut ctx = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .build();
    let env = crate::test_support::env_without_password(name);
    let project = env.with_file_name("stack");
    ctx.settings.stack_dir = Some(project.clone());
    ctx.settings.env_file = Some(env);
    for source in ["config/shelf", "mounts/shelf/cache", "config/reader"] {
        let _ = std::fs::create_dir_all(project.join(source));
    }
    if let Some(text) = text {
        let _ = std::fs::write(project.join(FILE), text);
    }
    (ctx, project)
}

/// `at` put back as a link to a directory outside the project.
#[cfg(unix)]
fn linked(at: &Path) {
    let outside = at.with_file_name(format!(
        "{}-outside",
        at.file_name().unwrap_or_default().to_string_lossy()
    ));
    let _ = std::fs::create_dir_all(&outside);
    let _ = std::fs::remove_dir_all(at);
    let _ = std::os::unix::fs::symlink(&outside, at);
}

#[test]
fn a_project_whose_sources_are_plain_starts() {
    let (ctx, _) = machine("linking-plain", Some(PROJECT));
    assert!(unlinked(&ctx).is_ok());
}

#[test]
fn a_machine_with_no_project_has_nothing_to_refuse() {
    let (ctx, _) = machine("linking-absent", None);
    assert!(unlinked(&ctx).is_ok());
    let bare = crate::test_support::a_context()
        .over(crate::test_support::embedded())
        .build();
    assert!(unlinked(&bare).is_ok());
}

#[cfg(unix)]
#[test]
fn a_link_at_a_services_own_directory_refuses_the_start_naming_it() {
    let (ctx, project) = machine("linking-own", Some(PROJECT));
    linked(&project.join("config").join("shelf"));
    let refused = unlinked(&ctx).err();
    assert_eq!(
        refused.as_ref().map(|problem| problem.code),
        Some(MOUNT_LINKED)
    );
    let said = refused.map(|problem| problem.summary).unwrap_or_default();
    assert!(said.starts_with("shelf was not started"), "{said}");
    assert!(
        said.contains(&project.join("config/shelf").display().to_string()),
        "{said}"
    );
}

#[cfg(unix)]
#[test]
fn a_link_on_the_way_to_a_mount_refuses_it_and_every_one_is_named() {
    let (ctx, project) = machine("linking-way", Some(PROJECT));
    linked(&project.join("mounts").join("shelf"));
    linked(&project.join("config").join("reader"));
    let detail = unlinked(&ctx)
        .err()
        .and_then(|problem| problem.detail)
        .unwrap_or_default();
    assert_eq!(
        detail,
        format!(
            "shelf: {}\nreader: {}",
            project.join("mounts/shelf/cache").display(),
            project.join("config/reader").display()
        )
    );
}

#[cfg(unix)]
#[test]
fn a_project_reached_through_a_link_refuses_every_start() {
    let (ctx, project) = machine("linking-project", None);
    let elsewhere = project.with_file_name("elsewhere.yml");
    let _ = std::fs::write(&elsewhere, PROJECT);
    let _ = std::os::unix::fs::symlink(&elsewhere, project.join(FILE));
    let said = unlinked(&ctx)
        .err()
        .map(|problem| problem.summary)
        .unwrap_or_default();
    assert!(said.starts_with("the stack was not started"), "{said}");
}
