use std::path::PathBuf;

use lemonfiber_fixtures::scratch::Scratch;

use super::{journalled, layered, unvetted, FILE};
use crate::app::Ctx;
use crate::test_support::{a_context, overlaid};

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
fn only_the_operators_overlay_is_layered_and_nothing_of_lemonfibers_own() {
    let (_dir, ctx, overlay) = machine("layered");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(layered(Some(&env)).is_empty());
    assert!(layered(None).is_empty());

    assert!(std::fs::write(env.with_file_name("beside.yml"), "services: {}\n").is_ok());
    assert!(
        layered(Some(&env)).is_empty(),
        "standing beside writes no overlay"
    );

    assert!(std::fs::write(&overlay, "services: {}\n").is_ok());
    assert_eq!(layered(Some(&env)), [overlay]);
}
