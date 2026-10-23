use lemonfiber_fixtures::scratch::Scratch;

use super::{forgotten, kept, manifest, root};
use crate::config::Settings;
use crate::plugin::Installed;
use crate::test_support::a_context;

fn installed() -> Option<Installed> {
    lemonfiber_plugin::Manifest::from_toml(crate::plugin::program::tests::RUNNING)
        .ok()
        .map(|manifest| Installed::of(&manifest))
}

#[test]
fn the_starting_files_of_a_run_proved_service_are_kept_beside_the_record_and_go_with_it() {
    let home = Scratch::new("kept-starting");
    let source = Scratch::new("kept-source");
    let _ = std::fs::create_dir_all(source.join("config/qualitysync/includes"));
    let _ = std::fs::write(source.join("config/qualitysync/includes/a.yml"), "a");
    let _ = std::fs::create_dir_all(source.join("config/other"));
    let _ = std::fs::write(source.join("config/other/b.yml"), "b");
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(home.join(".env")),
            ..Settings::default()
        })
        .build();
    let Some(installed) = installed() else {
        return;
    };
    kept(&ctx, &installed, &source);
    let at = root(&ctx, "quality");
    assert_eq!(at, Some(home.join("plugin-starting/quality")));
    let held = |path: &str| at.as_ref().map(|at| at.join(path).exists());
    assert_eq!(held("config/qualitysync/includes/a.yml"), Some(true));
    assert_eq!(held("config/other/b.yml"), Some(false));
    forgotten(&ctx, "quality");
    assert_eq!(at.map(|at| at.exists()), Some(false));
}

#[test]
fn with_nowhere_to_keep_them_nothing_is_kept() {
    let ctx = a_context().build();
    assert_eq!(root(&ctx, "quality"), None);
    if let Some(installed) = installed() {
        kept(
            &ctx,
            &installed,
            std::path::Path::new("/lemonfiber/no/such/source"),
        );
        assert_eq!(
            manifest(&ctx, &installed).err().as_deref(),
            Some("there is nowhere configured that its manifest is kept")
        );
    }
    forgotten(&ctx, "quality");
}

#[test]
fn where_the_place_to_keep_them_cannot_be_made_nothing_is_kept() {
    let home = Scratch::new("kept-blocked");
    let source = Scratch::new("kept-blocked-source");
    let _ = std::fs::write(source.join(lemonfiber_bundle::MANIFEST), "a");
    let _ = std::fs::write(home.join("plugin-starting"), "not a directory");
    let ctx = a_context()
        .settings(Settings {
            env_file: Some(home.join(".env")),
            ..Settings::default()
        })
        .build();
    let Some(installed) = installed() else {
        return;
    };
    kept(&ctx, &installed, &source);
    assert_eq!(
        root(&ctx, "quality").map(|at| at.join(lemonfiber_bundle::MANIFEST).exists()),
        Some(false)
    );
    assert!(manifest(&ctx, &installed).is_err());
}
