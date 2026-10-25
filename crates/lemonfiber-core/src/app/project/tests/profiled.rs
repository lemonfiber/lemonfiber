//! The files written beside the project, and what the overlay merged after it comes to.

use super::machine;

/// A project holding a stack service of the `dl` profile and a plugin's service.
const HOLDING: &str =
    "services:\n  sonarr:\n    profiles:\n    - dl\n  komga:\n    profiles:\n    - plugin-komga\n";

/// What the profiles file holds.
fn beside(project: &std::path::Path) -> String {
    std::fs::read_to_string(project.join(".env")).unwrap_or_default()
}

#[test]
fn a_start_names_its_profiles_and_a_later_write_keeps_them_beside_every_plugins() {
    let (ctx, project) = machine("profiled");
    assert!(super::super::kept(&ctx, HOLDING).is_ok());
    assert_eq!(
        beside(&project),
        "COMPOSE_PROFILES=plugin-komga\nCOMPOSE_FILE=compose.generated.yml\n"
    );

    let started = super::super::materialised_with(
        &ctx,
        &super::super::held(&ctx).unwrap_or_default(),
        super::super::Pass::Write,
        Some(&["media".to_owned()].into_iter().collect()),
    );
    assert!(started.is_ok(), "{started:?}");
    assert_eq!(
        beside(&project),
        "COMPOSE_PROFILES=media,plugin-komga\nCOMPOSE_FILE=compose.generated.yml\n"
    );

    let without = "services:\n  sonarr:\n    profiles:\n    - dl\n";
    assert!(super::super::kept(&ctx, without).is_ok());
    assert_eq!(
        beside(&project),
        "COMPOSE_PROFILES=media\nCOMPOSE_FILE=compose.generated.yml\n"
    );
}

/// A project holding a service of a plugin the bundle installs, written into the stack's
/// `media` profile, beside another plugin's service.
const MOVED: &str = "services:\n  jellyfin-adapter:\n    profiles:\n    - media\n  komga:\n    profiles:\n    - plugin-komga\n";

/// A plugin the bundle installs sits in a stack profile, so plain Compose starts it with
/// that profile as it does the stack's own services, and not as every plugin's is.
#[test]
fn a_service_in_a_stack_profile_is_started_with_that_profile_alone() {
    let (ctx, project) = machine("profiled-moved");
    assert!(super::super::kept(&ctx, MOVED).is_ok());
    assert_eq!(
        beside(&project),
        "COMPOSE_PROFILES=plugin-komga\nCOMPOSE_FILE=compose.generated.yml\n"
    );

    let started = super::super::materialised_with(
        &ctx,
        &super::super::held(&ctx).unwrap_or_default(),
        super::super::Pass::Write,
        Some(&["media".to_owned()].into_iter().collect()),
    );
    assert!(started.is_ok(), "{started:?}");
    assert_eq!(
        beside(&project),
        "COMPOSE_PROFILES=media,plugin-komga\nCOMPOSE_FILE=compose.generated.yml\n"
    );
}

#[test]
fn a_profiles_file_somebody_edited_is_kept() {
    let (ctx, project) = machine("profiled-edited");
    assert!(super::super::kept(&ctx, HOLDING).is_ok());
    assert!(std::fs::write(project.join(".env"), "COMPOSE_PROFILES=mine\n").is_ok());

    let started = super::super::materialised_with(
        &ctx,
        &super::super::held(&ctx).unwrap_or_default(),
        super::super::Pass::Write,
        Some(&["media".to_owned()].into_iter().collect()),
    );

    assert!(started.is_ok(), "{started:?}");
    assert_eq!(beside(&project), "COMPOSE_PROFILES=mine\n");
}

#[test]
fn the_operators_overlay_is_read_after_the_project() {
    let (mut ctx, project) = machine("profiled-overlay");
    let overlay = crate::overlay::at(&ctx.settings.env_file.clone().unwrap_or_default());
    assert!(std::fs::write(&overlay, "services: {}\n").is_ok());
    ctx.settings.overlays = vec![overlay.clone()];

    assert!(super::super::kept(&ctx, HOLDING).is_ok());

    assert_eq!(
        beside(&project),
        format!(
            "COMPOSE_PROFILES=plugin-komga\nCOMPOSE_FILE=compose.generated.yml:{}\n",
            overlay.display()
        )
    );
}

#[test]
fn an_overlay_mounting_the_data_location_again_is_read_with_the_project() {
    let (mut ctx, _) = machine("crowded-overlay");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(crate::config::store::set(&env, "DATA_ROOT", "/srv/media").is_ok());
    let overlay = crate::overlay::at(&env);
    assert!(std::fs::write(
        &overlay,
        "services:\n  sonarr:\n    volumes:\n    - ${DATA_ROOT}/downloads:/downloads\n"
    )
    .is_ok());
    ctx.settings.overlays = vec![overlay];
    let project = "services:\n  sonarr:\n    volumes:\n    - /srv/media:/data\n";
    assert!(super::super::kept(&ctx, project).is_ok());

    let crowded = super::super::crowded(&ctx);

    assert_eq!(
        crowded
            .iter()
            .map(|one| one.service.as_str())
            .collect::<Vec<_>>(),
        ["sonarr"]
    );
}

#[test]
fn an_overlay_whose_path_cannot_be_listed_leaves_plain_compose_reading_the_project() {
    let (mut ctx, project) = machine("profiled-colon");
    let overlay = project.with_file_name("over:lay.yml");
    assert!(std::fs::write(&overlay, "services: {}\n").is_ok());
    ctx.settings.overlays = vec![overlay];

    assert!(super::super::kept(&ctx, HOLDING).is_ok());

    assert_eq!(
        beside(&project),
        "COMPOSE_PROFILES=plugin-komga\nCOMPOSE_FILE=compose.generated.yml\n"
    );
}

#[test]
fn a_project_declared_unmanaged_is_neither_written_nor_profiled_beside() {
    let (mut ctx, project) = machine("profiled-unmanaged");
    ctx.settings.unmanaged = vec![
        (crate::project::FILE.to_owned(), "mine".to_owned()),
        (super::super::profiled::FILE.to_owned(), "mine".to_owned()),
    ];

    let written = super::super::materialised(&ctx, HOLDING, super::super::Pass::Write);

    assert!(written.is_ok_and(|edit| edit.is_none()));
    assert!(!project.join(crate::project::FILE).exists());
    assert!(!project.join(".env").exists());
}

#[cfg(unix)]
#[test]
fn a_project_reached_through_a_link_is_not_written_through() {
    let (ctx, project) = machine("profiled-linked");
    let elsewhere = project.with_file_name("elsewhere.yml");
    assert!(std::fs::write(&elsewhere, "theirs").is_ok());
    assert!(std::os::unix::fs::symlink(&elsewhere, project.join(crate::project::FILE)).is_ok());

    let written = super::super::materialised(&ctx, HOLDING, super::super::Pass::Write);

    assert!(written.is_err());
    assert_eq!(
        std::fs::read_to_string(&elsewhere).ok().as_deref(),
        Some("theirs")
    );
}

#[test]
fn rewriting_a_project_that_would_not_change_journals_nothing() {
    let (ctx, _) = machine("profiled-unchanged");
    let Ok(manifest) = ctx.stack.checked_manifest(ctx.today()) else {
        unreachable!("the carried stack reads")
    };

    let first = super::super::rewritten(&ctx, &manifest, &[], "plugin komga", "1");
    let second = super::super::rewritten(&ctx, &manifest, &[], "plugin komga", "2");

    assert!(first.is_ok() && second.is_ok(), "{first:?} {second:?}");
    assert_eq!(super::journal(&ctx).len(), 1);
}
