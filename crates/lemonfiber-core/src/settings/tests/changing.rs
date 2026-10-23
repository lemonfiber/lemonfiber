use std::path::{Path, PathBuf};
use std::sync::Arc;

use lemonfiber_fixtures::support::{Recording, Reporting};
use lemonfiber_plugin::Manifest;

use super::super::changing::{changed, Asked, REFUSING};
use crate::app::Ctx;
use crate::config::paths::SETTINGS;
use crate::error::Problem;
use crate::journal::{Change, Kind};
use crate::model::{SettingChange, SettingsReport};
use crate::plugin::{Installed, Register};
use crate::ports::docker::{Health, Lifecycle};
use crate::ports::process::Output;
use crate::test_support::{a_context, env_without_password, overlaid};

const MODE: &str = "comics:mode";

const PLUGIN: &str = r#"
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
KOMGA_MODE = { setting = "comics:mode", default = "slow" }
KOMGA_KEY  = { secret = "token" }

[[service]]
id          = "komga-worker"
name        = "Komga worker"
image       = "example.invalid/komga"
digest      = "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
tag         = "1.11.0"
criticality = "important"
config_path = "/config"

[service.env]
WORKER_MODE = { setting = "comics:mode", default = "slow" }

[[secret]]
id  = "token"
of  = "komga"
why = "Signs the library in"
"#;

pub(super) struct Machine {
    pub(super) ctx: Ctx,
    pub(super) runner: Arc<Recording>,
    pub(super) env: PathBuf,
    pub(super) document: PathBuf,
}

fn installed() -> Installed {
    let mut installing: Vec<Installed> = Manifest::from_toml(PLUGIN)
        .iter()
        .map(Installed::of)
        .collect();
    assert_eq!(installing.len(), 1);
    installing
        .pop()
        .unwrap_or_else(|| crate::test_support::an_installed("comics", Vec::new()))
}

pub(super) fn machine(name: &str, engine: Reporting) -> Machine {
    let env = env_without_password(&format!("changing-{name}"));
    let stack = env.with_file_name("data").join("stack");
    let comics = installed();
    let mut register = Register::empty();
    assert!(register.record(comics.clone()).is_ok());
    assert!(std::fs::write(
        env.with_file_name(crate::config::paths::PLUGINS),
        serde_json::to_string(&register).unwrap_or_default()
    )
    .is_ok());
    let runner = Arc::new(Recording::answering(Ok(Output {
        status: Some(0),
        stdout: String::new(),
        stderr: String::new(),
    })));
    let ctx = a_context()
        .runner(runner.clone())
        .engine(Arc::new(engine))
        .settings(crate::config::Settings {
            env_file: Some(env.clone()),
            stack_dir: Some(stack),
            ..crate::config::Settings::default()
        })
        .build();
    let generated = ctx
        .stack
        .checked_manifest(ctx.today())
        .ok()
        .map(|manifest| crate::app::project::current(&ctx, &manifest, register.installed()).text)
        .unwrap_or_default();
    assert!(crate::app::project::kept(&ctx, &generated).is_ok());
    let document = crate::app::project::path(&ctx).unwrap_or_default();
    Machine {
        ctx,
        runner,
        env,
        document,
    }
}

pub(super) fn running(services: &[&str]) -> Reporting {
    Reporting::holding(services, Lifecycle::Running, Health::Healthy)
}

pub(super) async fn asked(
    ctx: &Ctx,
    setting: &str,
    value: Option<&str>,
    agreement: Option<&str>,
) -> Result<SettingsReport, Box<Problem>> {
    changed(
        ctx,
        &Asked {
            setting,
            value,
            agreement,
        },
    )
    .await
}

pub(super) fn change(report: Result<SettingsReport, Box<Problem>>) -> Option<SettingChange> {
    report.ok().and_then(|report| report.change)
}

pub(super) async fn agreed(
    ctx: &Ctx,
    setting: &str,
    value: Option<&str>,
) -> Result<SettingsReport, Box<Problem>> {
    let offer = change(asked(ctx, setting, value, None).await)
        .map(|change| change.agreement)
        .unwrap_or_default();
    asked(ctx, setting, value, Some(&offer)).await
}

pub(super) fn journal(ctx: &Ctx) -> Vec<Change> {
    crate::app::targets::layout(ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| journal.changes().to_vec())
        .unwrap_or_default()
}

pub(super) fn code(report: &Result<SettingsReport, Box<Problem>>) -> Option<&str> {
    report.as_ref().err().map(|problem| problem.code.as_str())
}

pub(super) fn blocking_the_journal(ctx: &Ctx) -> PathBuf {
    let journal = crate::app::targets::layout(ctx)
        .map(|paths| paths.journal())
        .unwrap_or_default();
    let mut name = journal.file_name().unwrap_or_default().to_os_string();
    name.push(".writing");
    let blocked = journal.with_file_name(name);
    assert!(std::fs::create_dir_all(blocked.join("held")).is_ok());
    blocked
}

fn recorded(machine: &Machine, name: &str) -> Option<String> {
    crate::settings::Settings::at(&machine.env.with_file_name(SETTINGS))
        .ok()
        .and_then(|settings| settings.value(name).map(str::to_owned))
}

fn upped(runner: &Recording) -> Vec<Vec<String>> {
    runner
        .seen()
        .into_iter()
        .filter(|argv| argv.iter().any(|word| word == "up"))
        .collect()
}

#[tokio::test]
async fn a_rehearsal_names_every_entry_and_restart_and_writes_nothing() {
    let machine = machine("rehearsal", running(&["komga", "komga-worker"]));
    let before = std::fs::read_to_string(&machine.document).ok();

    let reading = change(asked(&machine.ctx, MODE, Some("fast"), None).await);
    let mut rehearsing = machine.ctx.clone();
    rehearsing.dry_run = true;
    let offer = reading
        .as_ref()
        .map(|one| one.agreement.clone())
        .unwrap_or_default();
    let rehearsed = asked(&rehearsing, MODE, Some("fast"), Some(&offer)).await;

    let names = ["komga".to_owned(), "komga-worker".to_owned()];
    assert_eq!(
        reading.as_ref().map(|one| one.regenerates.as_slice()),
        Some(names.as_slice())
    );
    assert_eq!(
        reading.as_ref().map(|one| one.restarts.as_slice()),
        Some(names.as_slice())
    );
    assert_eq!(reading.map(|one| one.applied), Some(false));
    assert!(rehearsed.as_ref().is_ok_and(|report| report.rehearsed));
    assert_eq!(change(rehearsed).map(|one| one.applied), Some(false));
    assert_eq!(std::fs::read_to_string(&machine.document).ok(), before);
    assert_eq!(recorded(&machine, MODE), None);
    assert!(journal(&machine.ctx).is_empty());
    assert!(machine.runner.seen().is_empty());
}

#[tokio::test]
async fn an_answered_change_is_one_journalled_run_written_after_its_record() {
    let machine = machine("answered", running(&["komga"]));
    let before = std::fs::read_to_string(&machine.document).unwrap_or_default();

    let made = change(agreed(&machine.ctx, MODE, Some("fast")).await);
    let written = std::fs::read_to_string(&machine.document).unwrap_or_default();
    let changes = journal(&machine.ctx);

    assert_eq!(made.as_ref().map(|one| one.applied), Some(true));
    assert_eq!(made.and_then(|one| one.unrestarted), None);
    assert_eq!(recorded(&machine, MODE).as_deref(), Some("fast"));
    assert!(written.contains("KOMGA_MODE: fast"), "{written}");
    assert!(written.contains("WORKER_MODE: fast"), "{written}");
    let stamps: std::collections::BTreeSet<&str> =
        changes.iter().map(|one| one.at.as_str()).collect();
    assert_eq!(stamps.len(), 1);
    let (run, started): (Vec<&Change>, Vec<&Change>) =
        changes.iter().partition(|one| one.operation == "settings");
    assert!(
        started
            .iter()
            .all(|one| one.operation == crate::app::project::OPERATION
                && matches!(one.kind, Kind::Made { .. })),
        "{started:?}"
    );
    assert!(run.iter().any(|one| one.kind
        == Kind::Setting {
            name: MODE.to_owned(),
            previous: None,
            current: Some("fast".to_owned()),
            filed: None,
        }));
    assert!(run.iter().any(|one| matches!(&one.kind,
        Kind::Rewritten { path, previous, .. }
            if *path == machine.document.display().to_string() && *previous == before)));
}

#[tokio::test]
async fn only_the_running_services_that_read_it_are_recreated() {
    let engine = running(&["komga", "sonarr"]).alongside(Reporting::holding(
        &["komga-worker"],
        Lifecycle::Exited,
        Health::None,
    ));
    let machine = machine("running", engine);

    let made = change(agreed(&machine.ctx, MODE, Some("fast")).await);

    assert_eq!(made.map(|one| one.restarts), Some(vec!["komga".to_owned()]));
    let upped = upped(&machine.runner);
    assert_eq!(upped.len(), 1, "{upped:?}");
    let argv = upped.concat();
    assert!(argv.iter().any(|word| word == "komga"), "{argv:?}");
    assert!(!argv.iter().any(|word| word == "komga-worker"), "{argv:?}");
    assert!(!argv.iter().any(|word| word == "sonarr"), "{argv:?}");
}

#[tokio::test]
async fn nothing_running_is_nothing_restarted() {
    let machine = machine("stopped", Reporting::absent());

    let made = change(agreed(&machine.ctx, MODE, Some("fast")).await);

    assert_eq!(made.map(|one| one.restarts), Some(Vec::new()));
    assert!(upped(&machine.runner).is_empty());
}

#[tokio::test]
async fn an_offer_answered_after_the_setting_moved_is_refused_naming_what_moved() {
    let machine = machine("moved", running(&["komga"]));
    let offer = change(asked(&machine.ctx, MODE, Some("fast"), None).await)
        .map(|one| one.agreement)
        .unwrap_or_default();
    assert!(crate::settings::store::recorded(
        &machine.env.with_file_name(SETTINGS),
        MODE,
        Some("medium")
    )
    .is_ok());

    let refused = asked(&machine.ctx, MODE, Some("fast"), Some(&offer)).await;

    assert_eq!(code(&refused), Some("SETTING-5"));
    assert!(refused
        .err()
        .is_some_and(|problem| problem.meaning.contains("what it holds now")));
    assert_eq!(recorded(&machine, MODE).as_deref(), Some("medium"));
    assert!(journal(&machine.ctx).is_empty());
}

#[tokio::test]
async fn an_offer_answered_after_a_reader_started_is_refused_naming_the_restarts() {
    let stopped = machine("started-since", Reporting::absent());
    let offer = change(asked(&stopped.ctx, MODE, Some("fast"), None).await)
        .map(|one| one.agreement)
        .unwrap_or_default();
    let started = machine("started-since", running(&["komga"]));

    let refused = asked(&started.ctx, MODE, Some("fast"), Some(&offer)).await;

    assert!(refused.err().is_some_and(|problem| problem.meaning
        == "Since it was read, the services it restarts changed, so nothing was changed. \
            Agreeing to it now would be agreeing to something nobody saw."));
}

#[tokio::test]
async fn a_document_edited_while_the_change_is_made_refuses_naming_it() {
    let machine = machine("edited", running(&["komga"]));
    let manifest = machine.ctx.stack.checked_manifest(machine.ctx.today()).ok();
    let register = crate::app::plugins::read(&machine.ctx).ok();
    let overwrites = manifest
        .as_ref()
        .zip(register.as_ref())
        .and_then(|(manifest, register)| {
            let after =
                crate::settings::Resolved::of(&machine.ctx, Some(manifest), register.installed())
                    .changed(MODE, Some("fast"));
            crate::app::plugins::writing::regenerated(
                &machine.ctx,
                manifest,
                register,
                &crate::app::targets::chosen_fillers(&machine.ctx),
                &after,
            )
            .ok()
        })
        .unwrap_or_default();
    assert!(std::fs::write(&machine.document, "edited by hand\n").is_ok());

    let written = crate::app::plugins::writing::overwritten(&machine.ctx, &overwrites, REFUSING);

    assert_eq!(overwrites.len(), 1);
    assert!(
        written.is_err_and(|problem| problem.code.as_str() == "SETTING-5"
            && problem
                .summary
                .contains(&machine.document.display().to_string()))
    );
    assert_eq!(
        std::fs::read_to_string(&machine.document).ok().as_deref(),
        Some("edited by hand\n")
    );
}

#[tokio::test]
async fn a_change_the_journal_will_not_take_is_not_made() {
    let machine = machine("unjournalled", running(&["komga"]));
    let before = std::fs::read_to_string(&machine.document).ok();
    let blocked = blocking_the_journal(&machine.ctx);

    let refused = agreed(&machine.ctx, MODE, Some("fast")).await;

    let _ = std::fs::remove_dir_all(&blocked);
    assert_eq!(code(&refused), Some("CONFIG-2"));
    assert_eq!(recorded(&machine, MODE), None);
    assert_eq!(std::fs::read_to_string(&machine.document).ok(), before);
    assert!(upped(&machine.runner).is_empty());
}

#[tokio::test]
async fn a_value_at_its_default_reads_as_recorded_and_clearing_takes_it_away() {
    let machine = machine("default", running(&[]));
    let read = |report: &SettingsReport| {
        report
            .settings
            .iter()
            .find(|one| one.name == MODE)
            .map(|one| (one.value.clone(), one.readers.clone()))
    };
    let listed =
        match crate::settings::run::settings(&machine.ctx, &crate::app::Settling::Read).await {
            Ok(crate::app::Outcome::Settings(report)) => read(&report),
            _ => None,
        };
    let (value, readers) = listed.unwrap_or_default();
    assert_eq!(value, None);
    assert!(readers
        .iter()
        .all(|reader| reader.default.as_deref() == Some("slow")));
    assert_eq!(readers.len(), 2);

    let at_default = agreed(&machine.ctx, MODE, Some("slow")).await;

    assert_eq!(
        at_default
            .as_ref()
            .ok()
            .and_then(read)
            .and_then(|(value, _)| value),
        Some("slow".to_owned())
    );
    assert_eq!(recorded(&machine, MODE).as_deref(), Some("slow"));
    let again = asked(&machine.ctx, MODE, Some("slow"), None).await;
    assert_eq!(code(&again), Some("SETTING-6"));

    let cleared = agreed(&machine.ctx, MODE, None).await;

    assert_eq!(
        cleared.as_ref().ok().and_then(read).map(|(value, _)| value),
        Some(None)
    );
    assert_eq!(recorded(&machine, MODE), None);
    let nothing = asked(&machine.ctx, MODE, None, None).await;
    assert_eq!(code(&nothing), Some("SETTING-6"));
}

#[tokio::test]
async fn a_secret_is_listed_without_its_value_and_never_written_here() {
    let machine = machine("secret", running(&[]));
    let at = crate::settings::secrets_at(&machine.ctx).unwrap_or_default();
    let keeping = [crate::settings::Keeping {
        name: "comics:token",
        value: "s3cr3t-value",
        from: crate::settings::Source::Recipe,
        at: 1,
    }];
    assert!(crate::settings::kept(&at, &keeping, &|_| Ok(())).is_ok());

    for name in [
        "comics:token",
        "gluetun:wireguard-key",
        crate::credential::VPN_PRIVATE_KEY,
    ] {
        let refused = asked(&machine.ctx, name, Some("x"), None).await;
        assert_eq!(code(&refused), Some("SETTING-2"), "{name}");
        assert!(refused
            .err()
            .is_some_and(|problem| problem.summary.contains(':')
                && problem
                    .remedies
                    .iter()
                    .any(|remedy| remedy.detail.as_deref().is_some_and(
                        |detail| detail.starts_with("lemonfiber credentials --rotate")
                    ))));
    }
    let listed = crate::settings::run::settings(&machine.ctx, &crate::app::Settling::Read).await;
    let text = format!("{listed:?}");
    assert!(!text.contains("s3cr3t-value"), "{text}");
    let secret = match listed {
        Ok(crate::app::Outcome::Settings(report)) => report
            .secrets
            .into_iter()
            .find(|one| one.name == "comics:token"),
        _ => None,
    };
    assert!(secret.is_some_and(|one| one.held
        && one.from == Some(crate::model::SecretFrom::Recipe)
        && one.readers.len() == 1));
    let entered = [crate::settings::Keeping {
        name: "comics:token",
        value: "typed-value",
        from: crate::settings::Source::Operator,
        at: 2,
    }];
    assert!(crate::settings::kept(&at, &entered, &|_| Ok(())).is_ok());
    let again = crate::settings::run::settings(&machine.ctx, &crate::app::Settling::Read).await;
    assert!(matches!(again, Ok(crate::app::Outcome::Settings(report))
        if report.secrets.iter().any(|one| one.name == "comics:token"
            && one.from == Some(crate::model::SecretFrom::Operator))));
    let minted = [crate::settings::Keeping {
        name: "comics:token",
        value: "minted-value",
        from: crate::settings::Source::Minted,
        at: 3,
    }];
    assert!(crate::settings::kept(&at, &minted, &|_| Ok(())).is_ok());
    let made = crate::settings::run::settings(&machine.ctx, &crate::app::Settling::Read).await;
    assert!(matches!(made, Ok(crate::app::Outcome::Settings(report))
        if report.secrets.iter().any(|one| one.name == "comics:token"
            && one.from == Some(crate::model::SecretFrom::Minted))));
}

#[tokio::test]
async fn setting_and_clearing_through_the_command_answers_with_the_settings() {
    let machine = machine("cleared", running(&[]));
    let set = crate::settings::run::settings(
        &machine.ctx,
        &crate::app::Settling::Set {
            setting: MODE.to_owned(),
            value: "fast".to_owned(),
            agreement: None,
        },
    )
    .await;
    assert!(matches!(set, Ok(crate::app::Outcome::Settings(report))
        if report.change.as_ref().is_some_and(|change| change.value.as_deref() == Some("fast")
            && !change.applied)));
    let made = agreed(&machine.ctx, MODE, Some("fast")).await;
    assert!(made.is_ok());
    let cleared = crate::settings::run::settings(
        &machine.ctx,
        &crate::app::Settling::Clear {
            setting: MODE.to_owned(),
            agreement: None,
        },
    )
    .await;
    assert!(matches!(cleared, Ok(crate::app::Outcome::Settings(report))
        if report.change.as_ref().is_some_and(|change| change.value.is_none() && !change.applied)));
}

#[cfg(unix)]
#[tokio::test]
async fn a_change_over_a_document_whose_mount_source_is_a_link_is_refused_naming_it() {
    let machine = machine("linked", running(&[]));
    let mut register =
        crate::app::plugins::read(&machine.ctx).unwrap_or_else(|_| Register::empty());
    let mut comics = installed();
    for placed in &mut comics.services {
        placed.fields.mount = vec![lemonfiber_manifest::container::Mount {
            from: "Caddyfile".to_owned(),
            to: "/etc/caddy/Caddyfile".to_owned(),
            ro: true,
        }];
    }
    register.forget("comics");
    assert!(register.record(comics).is_ok());
    assert!(std::fs::write(
        machine.env.with_file_name(crate::config::paths::PLUGINS),
        serde_json::to_string(&register).unwrap_or_default()
    )
    .is_ok());
    let stack = machine
        .document
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    for service in ["komga", "komga-worker"] {
        let tree = stack.join("mounts").join(service);
        let _ = std::fs::create_dir_all(&tree);
        let _ = std::os::unix::fs::symlink("/etc/hostname", tree.join("Caddyfile"));
    }
    let before = std::fs::read_to_string(&machine.document).ok();

    let refused = asked(&machine.ctx, MODE, Some("fast"), None).await;

    assert!(refused.is_err_and(|problem| problem.summary.contains("Caddyfile")));
    assert_eq!(std::fs::read_to_string(&machine.document).ok(), before);
}

#[tokio::test]
async fn a_name_nothing_reads_or_another_command_takes_is_refused_naming_it() {
    let machine = machine("unknown", running(&[]));
    for (name, wanted) in [
        ("comics:other", "SETTING-1"),
        ("elsewhere:mode", "SETTING-1"),
        ("VPN_COUNTRIES", "SETTING-1"),
        ("LEMONFIBER_IDEMPOTENCY_MINUTES", "SETTING-3"),
    ] {
        let refused = asked(&machine.ctx, name, Some("x"), None).await;
        assert_eq!(code(&refused), Some(wanted), "{name}");
        assert!(refused
            .err()
            .is_some_and(|problem| problem.summary.contains(name)));
    }
    let countries = asked(&machine.ctx, "VPN_COUNTRIES", Some("x"), None).await;
    assert!(countries.err().is_some_and(|problem| problem
        .remedies
        .iter()
        .any(|remedy| remedy.detail.as_deref()
            == Some("lemonfiber settings set gluetun:countries <value>"))));
}

#[tokio::test]
async fn a_value_that_cannot_be_written_is_refused() {
    let machine = machine("unwritable", running(&[]));
    for (name, value) in [
        (MODE, "two\nlines"),
        ("gluetun:countries", ""),
        (crate::settings::own::DEVICE_KEY_APPS, "android:nope"),
    ] {
        let refused = asked(&machine.ctx, name, Some(value), None).await;
        assert_eq!(code(&refused), Some("SETTING-4"), "{name}");
    }
}

#[tokio::test]
async fn a_core_setting_is_recorded_in_the_store_and_read_back_as_the_cores() {
    let machine = machine("core", running(&[]));
    let identities = format!(
        "android:app.example.lemon@{},ios:ABCDE12345.app.example.lemon",
        "a1".repeat(32)
    );

    let made = agreed(
        &machine.ctx,
        crate::settings::own::DEVICE_KEY_APPS,
        Some(&identities),
    )
    .await;

    let read = made.ok().and_then(|report| {
        report
            .settings
            .into_iter()
            .find(|one| one.name == crate::settings::own::DEVICE_KEY_APPS)
    });
    assert!(
        read.is_some_and(|one| one.whose == crate::model::Owner::Core
            && one.readers.is_empty()
            && one.value.as_deref() == Some(identities.as_str()))
    );
    assert_eq!(
        recorded(&machine, crate::settings::own::DEVICE_KEY_APPS),
        Some(identities)
    );
}

#[tokio::test]
async fn changing_the_device_key_apps_tells_the_operator_of_each_identity_gained_and_lost() {
    let machine = machine("device-key-apps", running(&[]));
    let identity = "ios:ABCDE12345.app.example.lemon";
    let set = agreed(
        &machine.ctx,
        crate::settings::own::DEVICE_KEY_APPS,
        Some(identity),
    )
    .await;
    assert!(set.is_ok());
    let cleared = agreed(&machine.ctx, crate::settings::own::DEVICE_KEY_APPS, None).await;
    assert!(cleared.is_ok());
    let told: Vec<String> = crate::app::outbox::load(&machine.ctx)
        .history()
        .into_iter()
        .map(|alert| alert.summary.clone())
        .collect();
    assert!(told.contains(&format!("The app {identity} may now register device keys")));
    assert!(told.contains(&format!(
        "The app {identity} may no longer register device keys"
    )));
}

#[tokio::test]
async fn a_stack_that_cannot_be_read_refuses_the_listing_and_the_change() {
    let mut machine = machine("unread-stack", running(&[]));
    machine.ctx.stack = crate::test_support::nowhere();

    let read = crate::settings::run::settings(&machine.ctx, &crate::app::Settling::Read).await;
    let changed = asked(&machine.ctx, MODE, Some("fast"), None).await;

    assert!(read.is_err());
    assert!(changed.is_err());
}

#[tokio::test]
async fn a_record_of_plugins_that_will_not_read_refuses_the_listing_and_the_change() {
    let machine = machine("unread-record", running(&[]));
    let kept = crate::app::plugins::kept_at(&machine.ctx).unwrap_or_default();
    assert!(std::fs::write(kept, "{ not json").is_ok());

    let read = crate::settings::run::settings(&machine.ctx, &crate::app::Settling::Read).await;
    let changed = asked(&machine.ctx, MODE, Some("fast"), None).await;

    assert!(matches!(read, Err(problem) if problem.code.as_str() == "PLUGIN-4"));
    assert_eq!(code(&changed), Some("PLUGIN-4"));
}

#[tokio::test]
async fn a_store_that_will_not_take_the_value_leaves_the_documents_as_they_were() {
    let machine = machine("unkept", running(&["komga"]));
    let before = std::fs::read_to_string(&machine.document).ok();
    let offer = change(asked(&machine.ctx, MODE, Some("fast"), None).await)
        .map(|one| one.agreement)
        .unwrap_or_default();
    assert!(std::fs::create_dir_all(machine.env.with_file_name(SETTINGS)).is_ok());

    let refused = asked(&machine.ctx, MODE, Some("fast"), Some(&offer)).await;

    assert!(refused.is_err());
    assert_eq!(std::fs::read_to_string(&machine.document).ok(), before);
    assert!(upped(&machine.runner).is_empty());
}

/// A change that writes the project without starting it journals the overlay the operator
/// wrote since, by content, as a start does.
#[tokio::test]
async fn a_change_that_writes_the_project_journals_the_overlay() {
    let machine = machine("overlaid", running(&[]));
    let overlay = crate::overlay::at(&machine.env);
    let written = "services:\n  komga:\n    mem_limit: 1g\n";
    assert!(std::fs::write(&overlay, written).is_ok());

    let made = agreed(&machine.ctx, MODE, Some("fast")).await;

    assert!(change(made).is_some_and(|change| change.applied));
    assert_eq!(
        overlaid(&machine.ctx),
        [(None, Some(crate::credential::fingerprint(written)))]
    );
}

#[tokio::test]
async fn changing_the_dashboards_search_address_applies_the_dashboard_again() {
    let machine = machine("dashboard-search", running(&[]));
    let homepage = crate::app::targets::project_directory(
        &machine.ctx.stack,
        machine.ctx.settings.stack_dir.as_deref(),
    )
    .unwrap_or_default()
    .join("config/homepage");
    let _ = std::fs::create_dir_all(&homepage);
    for file in ["services.yaml", "widgets.yaml"] {
        let _ = std::fs::write(homepage.join(file), "[]\n");
    }
    let search = "https://www.ecosia.org/search?q=";

    let set = agreed(
        &machine.ctx,
        crate::settings::own::DASHBOARD_SEARCH,
        Some(search),
    )
    .await;

    assert!(set.is_ok());
    let widgets = std::fs::read_to_string(homepage.join("widgets.yaml")).unwrap_or_default();
    assert!(widgets.contains(search), "{widgets}");
}
