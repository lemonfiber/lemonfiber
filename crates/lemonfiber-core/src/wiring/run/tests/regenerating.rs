//! A choice of filler regenerates the project in the same journalled change.

use super::{agreed, ctx, fill};
use crate::journal::{Change, Kind};
use crate::wiring::FILLS_KEY;

/// A media server a plugin brought, recorded as installed, with the project written as
/// lemonfiber last wrote it; answers where the project is.
fn a_plugin_media_server(ctx: &crate::app::Ctx) -> std::path::PathBuf {
    a_plugin_media_server_as(ctx, |_| ())
}

/// The same, its media server changed by `change` first.
fn a_plugin_media_server_as(
    ctx: &crate::app::Ctx,
    change: impl FnOnce(&mut crate::plugin::Placed),
) -> std::path::PathBuf {
    let api = lemonfiber_manifest::Api {
        kind: lemonfiber_manifest::ApiKind::Jellyfin,
        key_source: lemonfiber_manifest::KeySource::ConfigXml,
        path: Some("/config/config.xml".to_owned()),
        version: None,
    };
    let mut server =
        crate::test_support::a_placed("server", &["identity.source"], Some(api), Some(8097));
    change(&mut server);
    let installed = crate::test_support::an_installed("serving", vec![server]);
    let mut register = crate::plugin::Register::empty();
    assert!(register.record(installed).is_ok());
    assert!(
        crate::app::record::keep(crate::app::plugins::kept_at(ctx).as_deref(), &register).is_ok()
    );
    let generated = ctx
        .stack
        .checked_manifest(ctx.today())
        .ok()
        .map(|manifest| crate::app::project::current(ctx, &manifest, register.installed()).text)
        .unwrap_or_default();
    assert!(crate::app::project::kept(ctx, &generated).is_ok());
    crate::app::project::path(ctx).unwrap_or_default()
}

/// Every change the journal holds.
fn journal(ctx: &crate::app::Ctx) -> Vec<Change> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| journal.changes().to_vec())
        .unwrap_or_default()
}

/// What the project holds.
fn read(project: &std::path::Path) -> String {
    std::fs::read_to_string(project).unwrap_or_default()
}

/// The project the choices in `chosen` would regenerate, as the wiring would write it.
fn regenerated(
    ctx: &crate::app::Ctx,
    chosen: &str,
) -> Result<Vec<crate::app::plugins::writing::Overwrite>, Box<crate::error::Problem>> {
    let manifest = ctx
        .stack
        .checked_manifest(ctx.today())
        .map_err(|why| Box::new(crate::error::Diagnose::problem(&why)))?;
    let register = crate::app::plugins::read(ctx)?;
    crate::app::plugins::writing::regenerated(
        ctx,
        &manifest,
        &register,
        &crate::wiring::Chosen::read(Some(chosen)),
        &crate::settings::Resolved::of(ctx, Some(&manifest), register.installed()),
    )
}

/// Choosing a plugin's media server journals the setting and one rewrite of the project,
/// and nothing else; choosing the stack's next rewrites it once more, over what the first
/// choice wrote.
#[test]
fn a_choice_journals_the_setting_and_one_rewrite_of_the_project_and_nothing_else() {
    let (ctx, _) = ctx("regenerated");
    let project = a_plugin_media_server(&ctx);
    let before = read(&project);

    let chosen = agreed(&ctx, fill("identity.source", "server")).ok();

    assert_eq!(chosen.map(|report| report.applied), Some(true));
    let written = read(&project);
    assert_ne!(written, before);
    let changes = journal(&ctx);
    let rewrites: Vec<&Change> = changes
        .iter()
        .filter(|change| matches!(change.kind, Kind::Rewritten { .. }))
        .collect();
    assert!(
        matches!(rewrites.as_slice(), [one] if matches!(&one.kind,
            Kind::Rewritten { path, previous, .. }
                if *path == project.display().to_string() && *previous == before)),
        "{rewrites:?}"
    );
    assert!(changes.iter().all(|change| match &change.kind {
        Kind::Set { key, .. } => key == FILLS_KEY,
        Kind::Rewritten { .. } => true,
        _ => false,
    }));
    assert!(changes
        .iter()
        .all(|change| change.operation == crate::wiring::OPERATION));

    let back = agreed(&ctx, fill("identity.source", "jellyfin")).ok();

    assert_eq!(back.map(|report| report.applied), Some(true));
    let rewrites: Vec<Change> = journal(&ctx)
        .into_iter()
        .filter(|change| matches!(change.kind, Kind::Rewritten { .. }))
        .collect();
    assert!(
        matches!(rewrites.as_slice(), [_, again] if matches!(&again.kind,
            Kind::Rewritten { previous, .. } if *previous == written)),
        "{rewrites:?}"
    );
    assert_ne!(read(&project), written);
}

/// The project a choice would write is the one it already holds, so it is written over
/// with nothing.
#[test]
fn a_choice_the_project_already_holds_writes_over_nothing() {
    let (ctx, _) = ctx("unmoved");
    a_plugin_media_server(&ctx);

    let overwrites = regenerated(&ctx, "").ok();

    assert!(overwrites.is_some_and(|overwrites| overwrites.is_empty()));
}

/// With no project written yet there is nothing to write over: the next start writes it.
#[test]
fn with_no_project_yet_a_choice_writes_none() {
    let (ctx, _) = ctx("unwritten");
    let project = a_plugin_media_server(&ctx);
    assert!(std::fs::remove_file(&project).is_ok());

    let chosen = agreed(&ctx, fill("identity.source", "server")).ok();

    assert_eq!(chosen.map(|report| report.applied), Some(true));
    assert!(!project.exists());
}

/// A project somebody edited is not written over: the choice is refused, naming it, and
/// nothing is changed — not the project, and not the setting.
#[test]
fn an_edited_project_refuses_a_choice_naming_it() {
    let (ctx, at) = ctx("edited");
    let project = a_plugin_media_server(&ctx);
    assert!(std::fs::write(&project, "services: {}\n").is_ok());

    let refused = agreed(&ctx, fill("identity.source", "server")).err();

    assert!(
        refused.is_some_and(|problem| problem.code.as_str() == "STACK-11"
            && problem.summary.contains(&project.display().to_string())),
        "the project is named"
    );
    assert_eq!(read(&project), "services: {}\n");
    assert_eq!(super::recorded(&at), None);
}

/// The project changing between the moment a choice reads it and the moment it writes is
/// refused, with nothing written.
#[test]
fn a_project_changed_since_the_choice_read_it_is_not_written() {
    let (ctx, _) = ctx("raced");
    let project = a_plugin_media_server(&ctx);
    let overwrites = regenerated(&ctx, "identity.source=server").unwrap_or_default();
    assert!(std::fs::write(&project, "services: {}\n").is_ok());

    let written = crate::app::plugins::writing::overwritten(
        &ctx,
        &overwrites,
        crate::app::plugins::writing::WIRING,
    );

    assert!(!overwrites.is_empty());
    assert!(written.is_err());
    assert_eq!(read(&project), "services: {}\n");
}

/// A project the machine will not let a choice write over is refused as unwritten,
/// naming it, rather than reported written.
#[cfg(unix)]
#[test]
fn a_project_that_will_not_be_written_over_is_said_unwritten() {
    use std::os::unix::fs::PermissionsExt as _;

    let (ctx, _) = ctx("unwritable-over");
    let project = a_plugin_media_server(&ctx);
    let before = read(&project);
    let overwrites = regenerated(&ctx, "identity.source=server").unwrap_or_default();
    let holding = project
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let locked = std::fs::set_permissions(&holding, std::fs::Permissions::from_mode(0o500))
        .and_then(|()| std::fs::set_permissions(&project, std::fs::Permissions::from_mode(0o400)));

    let written = crate::app::plugins::writing::overwritten(
        &ctx,
        &overwrites,
        crate::app::plugins::writing::WIRING,
    );

    let _ = std::fs::set_permissions(&holding, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::set_permissions(&project, std::fs::Permissions::from_mode(0o600));
    assert!(locked.is_ok());
    assert_eq!(
        written
            .err()
            .map(|problem| problem.code.as_str().to_owned())
            .as_deref(),
        Some("WIRE-4")
    );
    assert_eq!(read(&project), before);
}

/// The same, asked through the choice itself.
#[cfg(unix)]
#[test]
fn a_choice_whose_project_will_not_be_written_over_is_refused_as_unwritten() {
    use std::os::unix::fs::PermissionsExt as _;

    let (ctx, _) = ctx("unwritable-choice");
    let project = a_plugin_media_server(&ctx);
    let before = read(&project);
    let holding = project
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let locked = std::fs::set_permissions(&holding, std::fs::Permissions::from_mode(0o500))
        .and_then(|()| std::fs::set_permissions(&project, std::fs::Permissions::from_mode(0o400)));

    let refused = agreed(&ctx, fill("identity.source", "server")).err();

    let _ = std::fs::set_permissions(&holding, std::fs::Permissions::from_mode(0o700));
    let _ = std::fs::set_permissions(&project, std::fs::Permissions::from_mode(0o600));
    assert!(locked.is_ok());
    assert_eq!(
        refused
            .map(|problem| problem.code.as_str().to_owned())
            .as_deref(),
        Some("WIRE-4")
    );
    assert_eq!(read(&project), before);
}

/// A choice that would write the project is refused, writing nothing, where a mount
/// source of a plugin's service is reached through a link.
#[cfg(unix)]
#[test]
fn a_choice_over_a_plugin_whose_mount_source_is_a_link_writes_nothing() {
    let (ctx, _) = ctx("linked");
    let project = a_plugin_media_server_as(&ctx, |server| {
        server.fields.mount = vec![lemonfiber_manifest::container::Mount {
            from: "Caddyfile".to_owned(),
            to: "/etc/caddy/Caddyfile".to_owned(),
            ro: true,
        }];
    });
    let tree = project
        .parent()
        .map(|directory| directory.join("mounts").join("server"))
        .unwrap_or_default();
    let _ = std::fs::create_dir_all(&tree);
    let _ = std::os::unix::fs::symlink("/etc/hostname", tree.join("Caddyfile"));
    let before = read(&project);

    let chosen = agreed(&ctx, fill("identity.source", "server"));

    assert!(
        chosen.is_err_and(|why| why.summary.contains("Caddyfile")),
        "the linked source is named"
    );
    assert_eq!(read(&project), before);
}

/// A choice over a data root the setting names that is refused writes no project from it,
/// and is refused naming it.
#[test]
fn a_choice_over_a_refused_data_root_is_refused_naming_it() {
    let (ctx, _) = ctx("rooted");
    let project = a_plugin_media_server(&ctx);
    let before = read(&project);
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(
        crate::config::store::set(&env, crate::config::DATA_ROOT_KEY, "nfs://nas local/x").is_ok()
    );

    let refused = regenerated(&ctx, "identity.source=server")
        .err()
        .map(|problem| problem.code);

    assert_eq!(refused, Some(crate::error::codes::storage::ROOT_REFUSED));
    assert_eq!(read(&project), before);
}

/// A choice, offered, rehearsed or made, names the operator's overlay it would be merged
/// under and every service it changes, as unvetted; with no overlay it names none.
#[test]
fn a_choice_names_the_unvetted_overlay_it_would_be_merged_under() {
    let (ctx, _) = ctx("unvetted");
    a_plugin_media_server(&ctx);
    let offered = super::substituting(&ctx, &fill("identity.source", "server")).ok();
    assert_eq!(offered.and_then(|report| report.unvetted), None);
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let overlay = crate::overlay::at(&env);
    assert!(std::fs::write(
        &overlay,
        "services:\n  jellyfin:\n    environment:\n      X: y\n"
    )
    .is_ok());

    let offered = super::substituting(&ctx, &fill("identity.source", "server")).ok();
    let made = agreed(&ctx, fill("identity.source", "server")).ok();

    for report in [offered, made] {
        assert_eq!(
            report.and_then(|report| report.unvetted),
            Some(crate::model::Unvetted {
                overlay: overlay.display().to_string(),
                services: vec!["jellyfin".to_owned()],
            })
        );
    }
}
