use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lemonfiber_manifest::container::Value;

use super::changing::{agreed, journal, machine, running};
use crate::app::Ctx;
use crate::config::paths::{SECRETS, SETTINGS};
use crate::config::store;
use crate::credential::fingerprint;
use crate::journal::Kind;
use crate::settings::legacy;

fn compose_reads() -> BTreeMap<(String, String), String> {
    let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/media-stack/compose");
    let mut reads = BTreeMap::new();
    for entry in std::fs::read_dir(at).into_iter().flatten().flatten() {
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let mut service: Option<&str> = None;
        for line in text.lines() {
            let indent = line.len() - line.trim_start().len();
            let trimmed = line.trim();
            if indent == 2 && trimmed.ends_with(':') {
                service = trimmed.strip_suffix(':');
                continue;
            }
            let read = trimmed
                .split_once(": ${")
                .and_then(|(variable, rest)| Some((variable, rest.split([':', '}']).next()?)));
            if let (Some(service), Some((variable, key))) = (service, read) {
                reads.insert((service.to_owned(), variable.to_owned()), key.to_owned());
            }
        }
    }
    reads
}

#[test]
fn each_setting_is_held_under_the_key_static_compose_reads_it_from() {
    let manifest = crate::test_support::a_context()
        .build()
        .stack
        .checked_manifest(crate::test_support::a_context().build().today());
    let reads = compose_reads();
    let mut checked = 0;
    for service in manifest
        .iter()
        .flat_map(|manifest| manifest.services.iter())
    {
        for (variable, value) in &service.env {
            let Value::Setting(setting) = value else {
                continue;
            };
            let read = reads.get(&(service.id.clone(), variable.clone()));
            assert_eq!(
                legacy::setting_key(&setting.setting),
                read.map(String::as_str),
                "{} reads {variable} from {read:?}",
                service.id
            );
            checked += 1;
        }
    }
    let settings = legacy::TABLE.iter().filter(|row| !row.secret).count();
    assert!(checked >= settings, "{checked} of {settings}");
    let named: std::collections::BTreeSet<&str> =
        legacy::TABLE.iter().map(|row| row.name).collect();
    let keyed: std::collections::BTreeSet<&str> = legacy::TABLE.iter().map(|row| row.key).collect();
    assert_eq!(named.len(), legacy::TABLE.len());
    assert_eq!(keyed.len(), legacy::TABLE.len());
}

#[tokio::test]
async fn every_legacy_setting_round_trips_through_its_environment_key() {
    let machine = machine("legacy", running(&[]));
    for row in legacy::TABLE.iter().filter(|row| !row.secret) {
        let value = format!("value-of-{}", row.key.to_lowercase());

        let made = agreed(&machine.ctx, row.name, Some(&value)).await;

        let held = store::read(&machine.env).ok();
        assert_eq!(
            held.as_ref().and_then(|file| file.get(row.key)),
            Some(value.as_str()),
            "{}",
            row.name
        );
        let read = made
            .ok()
            .and_then(|report| report.settings.into_iter().find(|one| one.name == row.name));
        assert!(
            read.as_ref()
                .is_some_and(|one| one.value.as_deref() == Some(value.as_str())
                    && one.environment.as_deref() == Some(row.key)
                    && !one.readers.is_empty()),
            "{read:?}"
        );
        assert!(journal(&machine.ctx).last().is_some_and(|last| last.kind
            == Kind::Set {
                key: row.key.to_owned(),
                previous: None,
                current: value.clone(),
            }));

        let cleared = agreed(&machine.ctx, row.name, None).await;

        assert_eq!(
            store::read(&machine.env)
                .ok()
                .and_then(|file| file.get(row.key).map(str::to_owned))
                .as_deref(),
            Some("")
        );
        assert!(cleared.ok().is_some_and(|report| report
            .settings
            .iter()
            .any(|one| one.name == row.name && one.value.is_none())));
    }
    assert!(
        crate::settings::Settings::at(&machine.env.with_file_name(SETTINGS))
            .is_ok_and(|settings| settings.value("gluetun:countries").is_none())
    );
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
