use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use lemonfiber_manifest::container::Value;

use super::changing::{agreed, journal, machine, running};
use crate::app::Ctx;
use crate::config::paths::{SECRETS, SETTINGS};
use crate::config::store;
use crate::credential::fingerprint;
use crate::journal::Kind;
use crate::settings::legacy;

/// Every key the template an operator's environment file was made from names.
fn template_keys() -> BTreeSet<String> {
    let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/media-stack/.env.example");
    std::fs::read_to_string(at)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim_start_matches(['#', ' ']).split_once('='))
        .map(|(key, _)| key.trim().to_owned())
        .collect()
}

#[test]
fn each_legacy_setting_is_filed_from_a_key_the_template_names_for_a_setting_the_stack_reads() {
    let ctx = crate::test_support::a_context().build();
    let manifest = ctx.stack.checked_manifest(ctx.today());
    let read: BTreeSet<String> = manifest
        .iter()
        .flat_map(|manifest| manifest.services.iter())
        .flat_map(|service| service.env.values())
        .filter_map(|value| match value {
            Value::Setting(setting) => Some(setting.setting.clone()),
            _ => None,
        })
        .collect();
    let template = template_keys();
    for row in &legacy::TABLE {
        assert!(
            template.contains(row.key),
            "{} is not in the template",
            row.key
        );
        assert!(
            row.secret || read.contains(row.name),
            "{} is read by no service",
            row.name
        );
    }
    let named: BTreeSet<&str> = legacy::TABLE.iter().map(|row| row.name).collect();
    let keyed: BTreeSet<&str> = legacy::TABLE.iter().map(|row| row.key).collect();
    assert_eq!(named.len(), legacy::TABLE.len());
    assert_eq!(keyed.len(), legacy::TABLE.len());
}

#[tokio::test]
async fn every_legacy_setting_is_recorded_in_the_store_and_never_in_the_environment_file() {
    let machine = machine("legacy", running(&[]));
    for row in legacy::TABLE.iter().filter(|row| !row.secret) {
        let value = format!("value-of-{}", row.key.to_lowercase());

        let made = agreed(&machine.ctx, row.name, Some(&value)).await;

        assert_eq!(
            setting(&machine.env, row.name).as_deref(),
            Some(value.as_str())
        );
        assert_eq!(line(&machine.env, row.key), None, "{}", row.name);
        let read = made
            .ok()
            .and_then(|report| report.settings.into_iter().find(|one| one.name == row.name));
        assert!(
            read.as_ref().is_some_and(
                |one| one.value.as_deref() == Some(value.as_str()) && !one.readers.is_empty()
            ),
            "{read:?}"
        );
        assert!(journal(&machine.ctx)
            .iter()
            .rev()
            .find(|change| matches!(change.kind, Kind::Setting { .. }))
            .is_some_and(|last| last.kind
                == Kind::Setting {
                    name: row.name.to_owned(),
                    previous: None,
                    current: Some(value.clone()),
                    filed: None,
                }));

        let cleared = agreed(&machine.ctx, row.name, None).await;

        assert_eq!(setting(&machine.env, row.name), None);
        assert_eq!(line(&machine.env, row.key), None, "{}", row.name);
        assert!(cleared.ok().is_some_and(|report| report
            .settings
            .iter()
            .any(|one| one.name == row.name && one.value.is_none())));
    }
}

fn stacked(name: &str, lines: &[(&str, &str)]) -> (Ctx, PathBuf) {
    let machine = machine(&format!("stacked-{name}"), running(&[]));
    for (key, value) in lines {
        assert!(store::set(&machine.env, key, value).is_ok());
    }
    (machine.ctx, machine.env)
}

fn setting(env: &Path, name: &str) -> Option<String> {
    crate::settings::Settings::at(&env.with_file_name(SETTINGS))
        .ok()
        .and_then(|settings| settings.value(name).map(str::to_owned))
}

fn line(env: &Path, key: &str) -> Option<String> {
    store::read(env)
        .ok()
        .and_then(|file| file.get(key).map(str::to_owned))
}

#[test]
fn the_stack_is_filed_once_into_the_stores_and_journalled_by_fingerprint() {
    let (ctx, env) = stacked(
        "once",
        &[
            ("VPN_COUNTRIES", "Belgium"),
            ("DOMAIN", "home.example"),
            ("UMASK", ""),
            ("HOMEPAGE_VAR_LAN_HOST", "10.0.0.2"),
            (crate::credential::VPN_PRIVATE_KEY, "wg-private-key"),
        ],
    );
    assert!(crate::settings::store::recorded(
        &env.with_file_name(SETTINGS),
        "lan-host",
        Some("kept")
    )
    .is_ok());

    assert!(crate::filed_stack(&ctx).is_ok());

    assert_eq!(
        setting(&env, "gluetun:countries").as_deref(),
        Some("Belgium")
    );
    assert_eq!(setting(&env, "domain").as_deref(), Some("home.example"));
    assert_eq!(setting(&env, "lan-host").as_deref(), Some("kept"));
    assert_eq!(setting(&env, "unpackerr:umask"), None);
    assert_eq!(
        crate::settings::Secrets::at(&env.with_file_name(SECRETS))
            .ok()
            .and_then(|kept| kept.value("gluetun:wireguard-key").map(str::to_owned))
            .as_deref(),
        Some("wg-private-key")
    );
    for key in [
        "VPN_COUNTRIES",
        "DOMAIN",
        "HOMEPAGE_VAR_LAN_HOST",
        crate::credential::VPN_PRIVATE_KEY,
    ] {
        assert_eq!(line(&env, key), None, "{key}");
    }
    assert_eq!(line(&env, "UMASK").as_deref(), Some(""));
    assert!(crate::settings::Settings::at(&env.with_file_name(SETTINGS))
        .is_ok_and(|settings| settings.is_stack_filed()));
    let changes = journal(&ctx);
    let kinds: Vec<&Kind> = changes.iter().map(|one| &one.kind).collect();
    assert_eq!(kinds.len(), 3, "{kinds:?}");
    assert!(kinds.contains(&&Kind::Setting {
        name: "gluetun:countries".to_owned(),
        previous: None,
        current: Some("Belgium".to_owned()),
        filed: Some("VPN_COUNTRIES".to_owned()),
    }));
    assert!(kinds.contains(&&Kind::Secret {
        name: "gluetun:wireguard-key".to_owned(),
        from: "operator".to_owned(),
        previous: None,
        current: Some(fingerprint("wg-private-key")),
        filed: Some(crate::credential::VPN_PRIVATE_KEY.to_owned()),
    }));
    let raw = std::fs::read_to_string(env.with_file_name(crate::config::paths::JOURNAL))
        .unwrap_or_default();
    assert!(!raw.contains("wg-private-key"), "{raw}");

    assert!(crate::filed_stack(&ctx).is_ok());
    assert_eq!(journal(&ctx).len(), 3, "a second run files nothing");
}

#[test]
fn a_run_stopped_before_the_marker_is_finished_by_the_next_with_nothing_filed_twice() {
    let (ctx, env) = stacked("stopped", &[("VPN_COUNTRIES", "Belgium")]);
    assert!(crate::filed_stack(&ctx).is_ok());
    let text = std::fs::read_to_string(env.with_file_name(SETTINGS)).unwrap_or_default();
    assert!(std::fs::write(
        env.with_file_name(SETTINGS),
        text.replace("\"stack_filed\": true", "\"stack_filed\": false")
    )
    .is_ok());
    assert!(crate::settings::Settings::at(&env.with_file_name(SETTINGS))
        .is_ok_and(|settings| !settings.is_stack_filed()));

    assert!(crate::filed_stack(&ctx).is_ok());

    assert_eq!(journal(&ctx).len(), 1);
    assert_eq!(
        setting(&env, "gluetun:countries").as_deref(),
        Some("Belgium")
    );
}

#[test]
fn a_rehearsal_files_nothing_of_the_stack() {
    let (ctx, env) = stacked("rehearsed", &[("VPN_COUNTRIES", "Belgium")]);
    assert!(crate::filed_stack(&ctx.clone().rehearsing()).is_ok());
    assert_eq!(line(&env, "VPN_COUNTRIES").as_deref(), Some("Belgium"));
    assert_eq!(setting(&env, "gluetun:countries"), None);
}

#[test]
fn undoing_a_filed_setting_puts_its_line_back() {
    let (ctx, env) = stacked("undone", &[("VPN_COUNTRIES", "Belgium")]);
    assert!(crate::filed_stack(&ctx).is_ok());

    let undone = super::super::undoing::resettled(
        &env,
        "gluetun:countries",
        None,
        Some("Belgium"),
        Some("VPN_COUNTRIES"),
    );

    assert!(matches!(undone, Ok(super::super::undoing::Undone::Done)));
    assert_eq!(setting(&env, "gluetun:countries"), None);
    assert_eq!(line(&env, "VPN_COUNTRIES").as_deref(), Some("Belgium"));
}

#[test]
fn a_stack_value_the_journal_will_not_take_is_not_filed() {
    for (name, key, value) in [
        (
            "secret",
            crate::credential::VPN_PRIVATE_KEY,
            "wg-private-key",
        ),
        ("setting", "VPN_COUNTRIES", "Belgium"),
    ] {
        let (ctx, env) = stacked(&format!("unjournalled-{name}"), &[(key, value)]);
        let blocked = super::changing::blocking_the_journal(&ctx);

        let filed = crate::filed_stack(&ctx);

        let _ = std::fs::remove_dir_all(&blocked);
        assert!(filed.is_err(), "{name}");
        assert_eq!(line(&env, key).as_deref(), Some(value), "{name}");
        assert_eq!(setting(&env, "gluetun:countries"), None, "{name}");
    }
}

#[test]
fn the_nas_overlays_server_and_export_are_folded_into_the_data_root_once() {
    let (ctx, env) = stacked(
        "nas",
        &[
            ("DATA_ROOT", "/mnt/nas"),
            ("NAS_HOST", "nas.local"),
            ("NAS_EXPORT", "/volume1/media"),
        ],
    );
    assert!(crate::filed_stack(&ctx).is_ok());

    assert_eq!(
        line(&env, "DATA_ROOT").as_deref(),
        Some("nfs://nas.local/volume1/media")
    );
    assert_eq!(line(&env, "NAS_HOST").as_deref(), Some(""));
    assert_eq!(line(&env, "NAS_EXPORT").as_deref(), Some(""));
    let set: Vec<(String, Option<String>, String)> = journal(&ctx)
        .into_iter()
        .filter_map(|change| match change.kind {
            Kind::Set {
                key,
                previous,
                current,
            } => Some((key, previous, current)),
            _ => None,
        })
        .collect();
    let owned = |key: &str, previous: &str, current: &str| {
        (
            key.to_owned(),
            Some(previous.to_owned()),
            current.to_owned(),
        )
    };
    assert_eq!(
        set,
        [
            owned("DATA_ROOT", "/mnt/nas", "nfs://nas.local/volume1/media"),
            owned("NAS_HOST", "nas.local", ""),
            owned("NAS_EXPORT", "/volume1/media", ""),
        ]
    );

    assert!(store::set(&env, "NAS_HOST", "other").is_ok());
    assert!(store::set(&env, "NAS_EXPORT", "/other").is_ok());
    assert!(crate::filed_stack(&ctx).is_ok());
    assert_eq!(
        line(&env, "DATA_ROOT").as_deref(),
        Some("nfs://nas.local/volume1/media"),
        "a second run folds nothing"
    );
}

#[test]
fn nothing_is_folded_where_the_nas_keys_are_partial_refused_or_already_folded() {
    for (name, lines) in [
        ("partial", vec![("NAS_HOST", "nas.local")]),
        (
            "refused",
            vec![("NAS_HOST", "nas,soft"), ("NAS_EXPORT", "/volume1")],
        ),
        (
            "folded",
            vec![
                ("DATA_ROOT", "nfs://other/export"),
                ("NAS_HOST", "nas.local"),
                ("NAS_EXPORT", "/volume1"),
            ],
        ),
    ] {
        let (ctx, env) = stacked(&format!("unfolded-{name}"), &lines);
        let before = line(&env, "DATA_ROOT");
        assert!(crate::filed_stack(&ctx).is_ok(), "{name}");
        assert_eq!(line(&env, "DATA_ROOT"), before, "{name}");
        assert!(journal(&ctx).is_empty(), "{name}");
    }
}

#[test]
fn a_fold_the_journal_will_not_take_is_not_made() {
    let (ctx, env) = stacked(
        "nas-unjournalled",
        &[("NAS_HOST", "nas.local"), ("NAS_EXPORT", "/volume1")],
    );
    let blocked = super::changing::blocking_the_journal(&ctx);

    let filed = crate::filed_stack(&ctx);

    let _ = std::fs::remove_dir_all(&blocked);
    assert!(filed.is_err());
    assert_eq!(line(&env, "DATA_ROOT"), None);
    assert_eq!(line(&env, "NAS_HOST").as_deref(), Some("nas.local"));
}

#[test]
fn stores_that_cannot_be_read_leave_the_file_as_it_is() {
    let (_, env) = stacked("unread-stores", &[("VPN_COUNTRIES", "Sweden")]);
    assert!(std::fs::write(env.with_file_name(SETTINGS), "not a store").is_ok());
    assert!(std::fs::write(env.with_file_name(SECRETS), "not a store").is_ok());
    let file = store::read(&env).unwrap_or_default();

    let overlaid = legacy::overlaid(&env, file.clone());
    let listed = legacy::listed(&env, file);

    assert_eq!(overlaid.get("VPN_COUNTRIES"), Some("Sweden"));
    assert_eq!(listed.get("VPN_COUNTRIES"), Some("Sweden"));
}
