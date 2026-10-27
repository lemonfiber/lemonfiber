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
        .map(|manifest| super::super::current(&ctx, manifest, &[]).text)
        .unwrap_or_default();

    let written = super::super::materialised(&ctx, &text, super::super::Pass::Write);
    let started = super::super::materialised_with(
        &ctx,
        &super::super::held(&ctx).unwrap_or_default(),
        super::super::Pass::Write,
        Some(&["media".to_owned()].into_iter().collect()),
    );

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

/// A key the operator replaces is written into the file that reads it, and into no other.
#[test]
fn a_replaced_key_is_written_into_every_file_that_reads_it() {
    const REPLACED: &str = "a-replacement-wireguard-key";
    let (ctx, _) = machine("secrets-replaced");
    let env = ctx.settings.env_file.clone().unwrap_or_default();
    let entered = |key: &str| {
        crate::settings::legacy::entered(&env, crate::credential::VPN_PRIVATE_KEY, key, 0)
            .is_some_and(|kept| kept.is_ok())
    };
    assert!(entered(KEY));
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK).ok();
    let resolved = crate::settings::Resolved::of(&ctx, manifest.as_ref(), &[]);
    let file = resolved.secrets_file("gluetun").unwrap_or_default();
    assert!(manifest
        .as_ref()
        .is_some_and(|manifest| super::super::secrets_written(&ctx, manifest, &[]).is_ok()));
    assert!(entered(REPLACED));

    let unread = crate::settings::files::reread(
        &ctx,
        manifest.as_ref(),
        &[],
        &resolved,
        &crate::plugin::Holding::Crossing("NOTHING_READS_THIS_KEY".to_owned()),
    );
    let still = std::fs::read_to_string(&file).unwrap_or_default();
    let reread = crate::settings::files::reread(
        &ctx,
        manifest.as_ref(),
        &[],
        &resolved,
        &crate::settings::files::holding_of(crate::credential::VPN_PRIVATE_KEY),
    );

    assert_eq!(unread.ok(), Some(Vec::new()));
    assert!(
        still.contains(&format!("WIREGUARD_PRIVATE_KEY={KEY}")),
        "{still}"
    );
    assert_eq!(reread.ok(), Some(vec!["gluetun".to_owned()]));
    assert!(std::fs::read_to_string(&file)
        .is_ok_and(|held| held.contains(&format!("WIREGUARD_PRIVATE_KEY={REPLACED}"))));
}

/// A stack service an installed first-party service stands in for gets no secrets file of
/// the stack's: the stand-in's own declaration is the only one that writes its file.
#[test]
fn a_stack_service_moved_in_place_is_no_longer_written_a_secrets_file() {
    let (ctx, _project) = machine("secrets-moved");
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK);
    let mut moved = crate::test_support::a_placed("gluetun", &[], None);
    moved.replaces = Some("gluetun".to_owned());
    let installed = [crate::test_support::pinned_as(
        crate::test_support::an_installed("gluetun", vec![moved]),
    )];

    let filed = |installed: &[crate::plugin::Installed]| -> Vec<String> {
        manifest
            .as_ref()
            .ok()
            .map(|manifest| {
                let resolved = crate::settings::Resolved::of(&ctx, Some(manifest), installed);
                crate::settings::files::every_file(Some(manifest), installed, &resolved)
                    .into_iter()
                    .filter(|(_, _, variables)| !variables.is_empty())
                    .map(|(service, _, _)| service)
                    .collect()
            })
            .unwrap_or_default()
    };

    assert!(filed(&[]).contains(&"gluetun".to_owned()));
    assert!(!filed(&installed).contains(&"gluetun".to_owned()));
}

/// What `service`'s secrets file holds, for the stack this build carries.
fn file_of(ctx: &crate::app::Ctx, service: &str) -> String {
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK).ok();
    crate::settings::Resolved::of(ctx, manifest.as_ref(), &[])
        .secrets_file(service)
        .and_then(|file| std::fs::read_to_string(file).ok())
        .unwrap_or_default()
}

/// An assembled part's key of a curator is that curator's own, read where lemonfiber keeps
/// it each time the part's file is written, never copied into the secret store, and
/// written again into every file that reads it once it is replaced.
#[test]
fn an_assembled_parts_crossing_key_is_read_from_its_holder_and_never_filed() {
    let (ctx, _) = machine("secrets-crossing");
    let recorded =
        |key: &str| crate::app::targets::record_secret(&ctx, "SONARR_API_KEY", key).is_ok();
    assert!(recorded("sonarrs-key"));
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK).ok();
    let resolved = crate::settings::Resolved::of(&ctx, manifest.as_ref(), &[]);
    assert!(manifest
        .as_ref()
        .is_some_and(|manifest| super::super::secrets_written(&ctx, manifest, &[]).is_ok()));

    assert!(file_of(&ctx, "unpackerr").contains("UN_SONARR_0_API_KEY=sonarrs-key"));
    assert!(file_of(&ctx, "recyclarr").contains("SONARR_API_KEY=sonarrs-key"));
    let filed = crate::settings::secrets_at(&ctx)
        .and_then(|at| std::fs::read_to_string(at).ok())
        .unwrap_or_default();
    assert!(!filed.contains("unpackerr:"), "{filed}");
    assert!(!filed.contains("recyclarr:"), "{filed}");

    assert!(recorded("sonarrs-new-key"));
    let reread = crate::settings::files::reread(
        &ctx,
        manifest.as_ref(),
        &[],
        &resolved,
        &crate::plugin::Holding::Crossing("LIBRARY_CURATE_TV_API_KEY".to_owned()),
    );

    let rewritten = reread.unwrap_or_default();
    assert!(rewritten.contains(&"unpackerr".to_owned()), "{rewritten:?}");
    assert!(rewritten.contains(&"recyclarr".to_owned()), "{rewritten:?}");
    assert!(file_of(&ctx, "unpackerr").contains("UN_SONARR_0_API_KEY=sonarrs-new-key"));
}

/// A curator the stack does not run holds no key to hand over, so the part's variable for
/// it is withheld rather than read under a name nothing keeps.
#[test]
fn an_assembled_parts_key_of_a_service_nothing_here_runs_is_withheld() {
    let (ctx, _) = machine("secrets-withheld");
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK)
        .ok()
        .map(|mut manifest| {
            manifest.services.retain(|service| service.id != "lidarr");
            manifest
        });
    let resolved = crate::settings::Resolved::of(&ctx, manifest.as_ref(), &[]);

    let held = manifest
        .as_ref()
        .and_then(|manifest| manifest.services.iter().find(|one| one.id == "unpackerr"))
        .map(|unpackerr| crate::settings::files::stack_holding(unpackerr, "music-key", &resolved));

    assert_eq!(held, Some(crate::plugin::Holding::Withheld));
}

/// A stack service's own key, one its secrets hold for no other service, is read under
/// the stack's name for the service itself.
#[test]
fn a_stack_services_own_key_is_read_under_its_own_name() {
    let (ctx, _) = machine("secrets-own");
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK).ok();
    let resolved = crate::settings::Resolved::of(&ctx, manifest.as_ref(), &[]);

    let held = manifest
        .as_ref()
        .and_then(|manifest| manifest.services.iter().find(|one| one.id == "unpackerr"))
        .cloned()
        .map(|mut unpackerr| {
            for secret in &mut unpackerr.secret {
                secret.of.clone_from(&unpackerr.id);
            }
            crate::settings::files::stack_holding(&unpackerr, "music-key", &resolved)
        });

    assert_eq!(
        held,
        Some(crate::plugin::Holding::Crossing(
            "ARCHIVE_EXTRACT_API_KEY".to_owned()
        ))
    );
}
