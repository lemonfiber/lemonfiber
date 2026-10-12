use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use lemonfiber_manifest::container::{SecretOf, Value};

use super::changing::{agreed, journal, machine, running};
use crate::app::Ctx;
use crate::config::paths::{SECRETS, SETTINGS};
use crate::config::store;
use crate::credential::{fingerprint, Reach};
use crate::journal::Kind;
use crate::settings::legacy;

fn compose_reads() -> BTreeMap<(String, String), String> {
    let at = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/media-stack/compose");
    let mut reads = BTreeMap::new();
    for entry in std::fs::read_dir(at).into_iter().flatten().flatten() {
        let text = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let read: serde_yaml_ng::Value = serde_yaml_ng::from_str(&text).unwrap_or_default();
        let Some(services) = read.get("services").and_then(|one| one.as_mapping()) else {
            continue;
        };
        for (service, body) in services {
            let environment = body.get("environment").and_then(|one| one.as_mapping());
            for (variable, value) in environment.into_iter().flatten() {
                let key = value
                    .as_str()
                    .and_then(|text| text.strip_prefix("${"))
                    .and_then(|text| text.split([':', '}']).next());
                if let (Some(service), Some(variable), Some(key)) =
                    (service.as_str(), variable.as_str(), key)
                {
                    reads.insert((service.to_owned(), variable.to_owned()), key.to_owned());
                }
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

#[tokio::test]
async fn replacing_a_crossing_credential_regenerates_and_restarts_what_reads_it() {
    let machine = machine("crossed", running(&["reader"]));
    let mut placed = crate::test_support::a_placed("reader", &[], None, None);
    placed.fields.env.insert(
        "SONARR_KEY".to_owned(),
        Value::Secret(SecretOf {
            secret: "sonarr-key".to_owned(),
        }),
    );
    let mut reader = crate::test_support::an_installed("reader", vec![placed]);
    reader.manifest = crate::test_support::CONTRACTED_MANIFEST.to_owned();
    reader.declared.secrets = vec![crate::plugin::Secret {
        id: "sonarr-key".to_owned(),
        of: "sonarr".to_owned(),
        why: "Asks the series service".to_owned(),
    }];
    assert!(store::set(&machine.env, "SONARR_API_KEY", "replacement-key").is_ok());
    let manifest = machine.ctx.stack.checked_manifest(machine.ctx.today()).ok();
    let installed = [reader];
    let resolved = crate::settings::Resolved::trusting(
        &machine.ctx,
        manifest.as_ref(),
        &installed,
        &crate::test_support::first_party("reader"),
    );

    let reached =
        crate::settings::run::reached(&machine.ctx, "SONARR_API_KEY", &installed, &resolved).await;
    let elsewhere =
        crate::settings::run::reached(&machine.ctx, "RADARR_API_KEY", &installed, &resolved).await;

    assert_eq!(reached.len(), 1);
    assert!(reached
        .iter()
        .all(|one| one.consumer == "reader" && one.reach == Reach::Updated));
    assert!(elsewhere.is_empty());
    let file = machine.env.with_file_name("env").join("reader.env");
    assert!(std::fs::read_to_string(file)
        .is_ok_and(|text| text.contains("SONARR_KEY=") && text.contains("replacement-key")));
    assert!(machine.runner.seen().iter().any(
        |argv| argv.iter().any(|word| word == "up") && argv.iter().any(|word| word == "reader")
    ));
}
