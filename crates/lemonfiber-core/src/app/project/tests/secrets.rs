//! A stack service's secrets, written into a file of its own that no container mounts,
//! and never into the project.

use super::machine;

/// The key the operator entered for the egress guard.
const KEY: &str = "wireguard-private-key-value";

#[cfg(unix)]
#[test]
fn a_secret_is_written_only_into_its_services_own_file_and_never_into_the_project() {
    use std::os::unix::fs::PermissionsExt as _;

    let (ctx, project) = machine("secrets");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(
        crate::settings::legacy::entered(&env, crate::credential::VPN_PRIVATE_KEY, KEY, 0)
            .is_some_and(|kept| kept.is_ok())
    );
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK);
    let manifest = manifest.as_ref().ok();

    let text = manifest
        .map(|manifest| {
            super::super::generated(
                manifest,
                &[],
                &crate::wiring::Chosen::read(None),
                &crate::settings::Resolved::of(&ctx, Some(manifest), &[]),
            )
            .text
        })
        .unwrap_or_default();
    let written = manifest.map(|manifest| super::super::secrets_written(&ctx, manifest, &[]));

    let file = manifest
        .map(|manifest| crate::settings::Resolved::of(&ctx, Some(manifest), &[]))
        .and_then(|resolved| resolved.secrets_file("gluetun"))
        .unwrap_or_default();
    assert!(written.is_some_and(|written| written.is_ok()));
    assert!(!text.contains(KEY), "the project holds no secret");
    assert!(text.contains(&file.display().to_string()), "{text}");
    assert!(!file.starts_with(&project), "no container mounts it");
    assert!(std::fs::read_to_string(&file)
        .is_ok_and(|held| held.contains(&format!("WIREGUARD_PRIVATE_KEY={KEY}"))));
    assert_eq!(
        std::fs::metadata(&file)
            .map(|meta| meta.permissions().mode() & 0o777)
            .ok(),
        Some(0o600)
    );
}

/// The operator's values the project is written with, each as their environment file
/// holds it.
const OPERATOR: [(&str, &str); 5] = [
    ("PUID", "1001"),
    ("PGID", "1002"),
    ("TZ", "Pacific/Auckland"),
    ("DATA_ROOT", "/srv/media"),
    ("LAN_BIND", "192.168.1.5"),
];

#[test]
fn the_operators_values_are_written_in_and_no_secret_reaches_the_project_or_its_profiles() {
    let (ctx, project) = machine("literal");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    for (key, value) in OPERATOR {
        assert!(crate::config::store::set(&env, key, value).is_ok());
    }
    assert!(crate::config::store::set(&env, "WIREGUARD_PRIVATE_KEY", KEY).is_ok());
    assert!(
        crate::settings::legacy::entered(&env, crate::credential::VPN_PRIVATE_KEY, KEY, 0)
            .is_some_and(|kept| kept.is_ok())
    );
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK);
    let manifest = manifest.as_ref().ok();
    let text = manifest
        .and_then(|manifest| super::super::current(&ctx, manifest).ok())
        .map(|project| project.text)
        .unwrap_or_default();

    let written = super::super::materialised(&ctx, &text, super::super::Pass::Write);
    let started = super::super::started(&ctx, &["media".to_owned()].into_iter().collect());

    assert!(
        written.is_ok() && started.is_ok(),
        "{written:?} {started:?}"
    );
    let on_disk = std::fs::read_to_string(project.join(crate::project::FILE)).unwrap_or_default();
    let beside = std::fs::read_to_string(project.join(".env")).unwrap_or_default();
    assert!(on_disk.contains("TZ: Pacific/Auckland"), "{on_disk}");
    assert!(on_disk.contains("user: 1001:1002"), "{on_disk}");
    assert!(on_disk.contains("PGID: '1002'"), "{on_disk}");
    assert!(on_disk.contains("/srv/media:/data"), "{on_disk}");
    assert!(on_disk.contains("192.168.1.5:"), "{on_disk}");
    assert!(
        !on_disk.contains("${"),
        "nothing is left for anything to fill"
    );
    assert_eq!(
        beside,
        "COMPOSE_PROFILES=media\nCOMPOSE_FILE=compose.generated.yml\n"
    );
    assert!(
        !on_disk.contains(KEY) && !beside.contains(KEY),
        "no secret is written in"
    );
}

#[test]
fn the_render_group_the_operator_recorded_is_what_the_project_is_written_with() {
    let (ctx, _) = machine("render-group");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    assert!(crate::config::store::set(&env, crate::config::RENDER_GROUP_KEY, " 44 ").is_ok());

    let group =
        crate::settings::Resolved::of(&ctx, None, &[]).project(|reading| reading.render_group);

    assert_eq!(group, Some(44));
}
