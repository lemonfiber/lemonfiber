use std::path::{Path, PathBuf};

use lemonfiber_plugin::Manifest;

use crate::app::Ctx;
use crate::config::store;
use crate::credential::fingerprint;
use crate::journal::Kind;
use crate::plugin::{Installed, Register};
use crate::test_support::{a_context, env_at};

fn key() -> String {
    crate::config::for_plugin("comics", "komga", crate::config::CAPTURED_SUFFIX)
}

const SECRET: &str = r#"
schema_version = 1

[plugin]
id          = "comics"
name        = "Comics"
version     = "1.0.0"
description = "Reads your comics"
without_it  = "Comics on disk"
upstream    = "https://example.invalid"
license     = "MIT"
forms       = ["library"]

[[service]]
id          = "komga"
name        = "Komga"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
port        = 25600
bind        = "lan"
criticality = "important"
takes_data  = true
config_path = "/config"

[service.env]
KOMGA_KEY = { secret = "komga" }

[[secret]]
id  = "komga"
of  = "komga"
why = "Signs the library in"
"#;

fn installed() -> Vec<Installed> {
    Manifest::from_toml(SECRET)
        .iter()
        .map(Installed::of)
        .collect()
}

fn machine(name: &str, value: Option<&str>) -> Ctx {
    let env = env_at(&format!("filing-{name}"), "pw");
    let mut register = Register::empty();
    let installing = installed();
    assert_eq!(installing.len(), 1);
    for one in installing {
        assert!(register.record(one).is_ok());
    }
    assert!(std::fs::write(
        env.with_file_name(crate::config::paths::PLUGINS),
        serde_json::to_string(&register).unwrap_or_default()
    )
    .is_ok());
    if let Some(value) = value {
        assert!(store::set(&env, &key(), value).is_ok());
    }
    a_context()
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            stack_dir: Some(env.with_file_name("data").join("stack")),
            ..crate::config::Settings::default()
        })
        .build()
}

fn env(ctx: &Ctx) -> PathBuf {
    ctx.settings.env_file.clone().unwrap_or_default()
}

fn line(ctx: &Ctx) -> Option<String> {
    store::read(&env(ctx))
        .ok()
        .and_then(|file| file.get(&key()).map(str::to_owned))
}

fn held(ctx: &Ctx) -> Option<String> {
    crate::settings::secrets_at(ctx)
        .and_then(|at| crate::settings::Secrets::at(&at).ok())
        .and_then(|kept| kept.value("comics:komga").map(str::to_owned))
}

fn filed_marker(ctx: &Ctx) -> bool {
    crate::settings::Settings::at(&env(ctx).with_file_name("settings.json"))
        .is_ok_and(|settings| settings.is_filed())
}

fn journal(ctx: &Ctx) -> Vec<crate::journal::Change> {
    crate::app::recover::journal_at(&env(ctx).with_file_name(crate::config::paths::JOURNAL))
        .unwrap_or_default()
        .changes()
        .to_vec()
}

fn raw_journal(ctx: &Ctx) -> String {
    std::fs::read_to_string(env(ctx).with_file_name(crate::config::paths::JOURNAL))
        .unwrap_or_default()
}

#[test]
fn a_plugins_secret_in_the_environment_file_is_filed_once_and_journalled() {
    let ctx = machine("once", Some("s3cr$t"));
    super::super::filing::filed(&ctx);
    assert_eq!(held(&ctx).as_deref(), Some("s3cr$t"));
    assert_eq!(line(&ctx), None, "its line is gone");
    assert!(filed_marker(&ctx));
    let changes = journal(&ctx);
    assert_eq!(changes.len(), 1, "{changes:?}");
    assert_eq!(
        changes
            .first()
            .map(|change| (&change.operation, &change.kind)),
        Some((
            &crate::settings::OPERATION.to_owned(),
            &Kind::Secret {
                name: "comics:komga".to_owned(),
                from: "recipe".to_owned(),
                previous: None,
                current: Some(fingerprint("s3cr$t")),
                filed: Some(key()),
            }
        ))
    );
    assert!(
        !raw_journal(&ctx).contains("s3cr$t"),
        "{}",
        raw_journal(&ctx)
    );
    let file = env(&ctx).with_file_name("env").join("komga.env");
    assert_eq!(
        std::fs::read_to_string(file).ok().as_deref(),
        Some("KOMGA_KEY=\"s3cr\\$t\"\n")
    );
    super::super::filing::filed(&ctx);
    assert_eq!(journal(&ctx).len(), 1, "a second run files nothing");
}

#[test]
fn an_empty_value_is_not_filed() {
    let ctx = machine("empty", Some(""));
    super::super::filing::filed(&ctx);
    assert_eq!(held(&ctx), None);
    assert_eq!(line(&ctx).as_deref(), Some(""));
    assert!(journal(&ctx).is_empty());
}

#[test]
fn a_value_already_held_is_never_overwritten() {
    let ctx = machine("held", Some("old"));
    let at = crate::settings::secrets_at(&ctx).unwrap_or_default();
    let keeping = [crate::settings::Keeping {
        name: "comics:komga",
        value: "kept",
        from: crate::settings::Source::Operator,
        at: 1,
    }];
    assert!(crate::settings::kept(&at, &keeping, &|_| Ok(())).is_ok());
    super::super::filing::filed(&ctx);
    assert_eq!(held(&ctx).as_deref(), Some("kept"));
    assert_eq!(line(&ctx), None);
    assert!(journal(&ctx).is_empty(), "nothing was filed to journal");
}

#[test]
fn a_rehearsal_files_nothing() {
    let ctx = machine("rehearsed", Some("s"));
    super::super::filing::filed(&ctx.clone().rehearsing());
    assert_eq!(held(&ctx), None);
    assert_eq!(line(&ctx).as_deref(), Some("s"));
}

#[test]
fn the_marker_is_written_last_so_a_run_stopped_before_it_is_finished_by_the_next() {
    let ctx = machine("stopped", Some("s"));
    let settings = env(&ctx).with_file_name("settings.json");
    let blocked = staging(&settings);
    assert!(std::fs::create_dir_all(blocked.join("held")).is_ok());
    super::super::filing::filed(&ctx);
    assert_eq!(held(&ctx).as_deref(), Some("s"));
    assert_eq!(line(&ctx), None);
    assert!(!filed_marker(&ctx), "the marker did not land");
    let _ = std::fs::remove_dir_all(&blocked);
    super::super::filing::filed(&ctx);
    assert_eq!(held(&ctx).as_deref(), Some("s"));
    assert_eq!(journal(&ctx).len(), 1);
}

#[test]
fn a_secret_that_could_not_be_journalled_is_not_filed() {
    let ctx = machine("unjournalled", Some("s"));
    let journal = env(&ctx).with_file_name(crate::config::paths::JOURNAL);
    let blocked = staging(&journal);
    assert!(std::fs::create_dir_all(blocked.join("held")).is_ok());
    super::super::filing::filed(&ctx);
    let _ = std::fs::remove_dir_all(&blocked);
    assert_eq!(held(&ctx), None);
    assert_eq!(line(&ctx).as_deref(), Some("s"));
}

fn staging(record: &Path) -> PathBuf {
    let mut name = record.file_name().unwrap_or_default().to_os_string();
    name.push(".writing");
    record.with_file_name(name)
}
