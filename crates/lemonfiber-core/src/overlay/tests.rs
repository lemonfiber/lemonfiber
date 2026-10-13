use std::path::PathBuf;

use lemonfiber_fixtures::scratch::Scratch;

use super::{changed, journalled, layered, unvetted, FILE};
use crate::app::Ctx;
use crate::config::env::EnvFile;
use crate::journal::Kind;
use crate::test_support::a_context;

fn machine(name: &str) -> (Scratch, Ctx, PathBuf) {
    let dir = Scratch::new(&format!("overlay-{name}"));
    let config = dir.join("config");
    let _ = std::fs::create_dir_all(&config);
    let env = config.join(".env");
    let ctx = a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            stack_dir: Some(dir.join("data").join("stack")),
            ..crate::config::Settings::default()
        })
        .build();
    (dir, ctx, config.join(FILE))
}

fn overlaid(ctx: &Ctx) -> Vec<(Option<String>, Option<String>)> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| {
            journal
                .changes()
                .iter()
                .filter_map(|change| match &change.kind {
                    Kind::Overlaid {
                        previous, current, ..
                    } => Some((previous.clone(), current.clone())),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_service_the_overlay_changes_is_named_and_nothing_else() {
    assert_eq!(
        changed("services:\n  sonarr:\n    ports: [\"1:2\"]\n  radarr: {}\nvolumes:\n  x: {}\n"),
        ["sonarr", "radarr"]
    );
    assert!(changed("volumes:\n  x: {}\n").is_empty());
    assert!(changed("{ not yaml").is_empty());
}

#[test]
fn the_overlay_is_named_unvetted_where_the_operator_wrote_one() {
    let (_dir, ctx, overlay) = machine("unvetted");
    assert_eq!(unvetted(&ctx), None);

    assert!(std::fs::write(&overlay, "services:\n  sonarr: {}\n").is_ok());
    let named = unvetted(&ctx);
    assert_eq!(
        named.as_ref().map(|one| one.overlay.clone()),
        Some(overlay.display().to_string())
    );
    assert_eq!(
        named.map(|one| one.services),
        Some(vec!["sonarr".to_owned()])
    );
}

#[test]
fn each_change_to_the_overlay_is_journalled_once_by_content() {
    let (_dir, ctx, overlay) = machine("journalled");
    assert!(journalled(&ctx).is_ok());
    assert!(overlaid(&ctx).is_empty(), "no overlay, nothing to say");

    assert!(std::fs::write(&overlay, "services:\n  sonarr: {}\n").is_ok());
    assert!(journalled(&ctx).is_ok());
    assert!(journalled(&ctx).is_ok());
    let written = crate::credential::fingerprint("services:\n  sonarr: {}\n");
    assert_eq!(overlaid(&ctx), [(None, Some(written.clone()))]);

    assert!(std::fs::write(&overlay, "services:\n  radarr: {}\n").is_ok());
    assert!(journalled(&ctx).is_ok());
    let changed = crate::credential::fingerprint("services:\n  radarr: {}\n");
    assert_eq!(
        overlaid(&ctx).last(),
        Some(&(Some(written), Some(changed.clone())))
    );

    assert!(std::fs::remove_file(&overlay).is_ok());
    assert!(journalled(&ctx).is_ok());
    assert_eq!(overlaid(&ctx).last(), Some(&(Some(changed), None)));
    assert_eq!(overlaid(&ctx).len(), 3);
}

#[test]
fn a_rehearsal_journals_nothing_of_the_overlay() {
    let (_dir, ctx, overlay) = machine("rehearsed");
    assert!(std::fs::write(&overlay, "services: {}\n").is_ok());
    assert!(journalled(&ctx.clone().rehearsing()).is_ok());
    assert!(overlaid(&ctx).is_empty());
}

#[test]
fn the_beside_moves_are_layered_first_and_the_operators_overlay_last() {
    let (_dir, ctx, overlay) = machine("layered");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let beside = env.with_file_name("beside.yml");
    let none = EnvFile::parse("");
    let moves = EnvFile::parse("LEMONFIBER_BESIDE=sonarr:8989=18989\n");
    let retired = EnvFile::parse(&format!("LEMONFIBER_OVERLAY={}\n", beside.display()));
    assert!(layered(Some(&env), &none).is_empty());
    assert!(layered(None, &moves).is_empty());

    assert!(std::fs::write(&beside, "services: {}\n").is_ok());
    assert!(layered(Some(&env), &none).is_empty(), "no moves recorded");
    assert!(
        layered(Some(&env), &retired).is_empty(),
        "the retired key is not read"
    );
    assert_eq!(layered(Some(&env), &moves), std::slice::from_ref(&beside));

    assert!(std::fs::write(&overlay, "services: {}\n").is_ok());
    assert_eq!(layered(Some(&env), &moves), [beside, overlay.clone()]);
    assert_eq!(layered(Some(&env), &none), [overlay]);
}
