use std::sync::Arc;

use lemonfiber_fixtures::support::{Recording, Reporting};
use lemonfiber_manifest::container::{SecretOf, Value};

use super::changing::{agreed, code, machine, running, Machine};
use crate::config::store;
use crate::credential::{Propagation, Reach};
use crate::model::SecretFrom;
use crate::plugin::Installed;
use crate::ports::process::{Failure, Output};
use crate::settings::Resolved;

const CROSSING: &str = "SONARR_API_KEY";

/// `installed` recorded as the one plugin installed on `machine`.
fn registered(machine: &Machine, installed: Installed) {
    let mut register = crate::plugin::Register::empty();
    assert!(register.record(installed).is_ok());
    assert!(std::fs::write(
        machine.env.with_file_name(crate::config::paths::PLUGINS),
        serde_json::to_string(&register).unwrap_or_default()
    )
    .is_ok());
}

fn crossed(name: &str, engine: Reporting) -> (Machine, [Installed; 1]) {
    let machine = machine(name, engine);
    assert!(store::set(&machine.env, CROSSING, "replacement-key").is_ok());
    (machine, [crate::test_support::a_crossing_reader()])
}

fn trusted(machine: &Machine, installed: &[Installed]) -> Resolved {
    let manifest = machine.ctx.stack.checked_manifest(machine.ctx.today()).ok();
    Resolved::trusting(
        &machine.ctx,
        manifest.as_ref(),
        installed,
        &crate::test_support::first_party("reader"),
    )
}

async fn reached(machine: &Machine, installed: &[Installed], setting: &str) -> Vec<Propagation> {
    crate::settings::run::reached(
        &machine.ctx,
        setting,
        installed,
        &trusted(machine, installed),
    )
    .await
}

fn answering(machine: &mut Machine, answer: Result<Output, Failure>) {
    machine.ctx.seams.runner = Arc::new(Recording::answering(answer));
}

fn failed(reached: &[Propagation], saying: &str) -> bool {
    reached.len() == 2
        && reached
            .iter()
            .filter(|one| one.consumer == "reader")
            .all(|one| matches!(&one.reach, Reach::Failed { detail } if detail.contains(saying)))
}

#[tokio::test]
async fn replacing_a_crossing_credential_regenerates_and_restarts_what_reads_it() {
    let (machine, installed) = crossed("crossed", running(&["reader"]));

    let reached_now = reached(&machine, &installed, CROSSING).await;
    let elsewhere = reached(&machine, &installed, "RADARR_API_KEY").await;

    assert_eq!(reached_now.len(), 2);
    assert!(reached_now
        .iter()
        .any(|one| one.consumer == "reader" && one.reach == Reach::Updated));
    assert!(reached_now.iter().any(|one| one.consumer == "reader-worker"
        && one.reach
            == Reach::Pending {
                detail: "lemonfiber up --service reader-worker".to_owned()
            }));
    assert!(elsewhere.is_empty());
    let file = machine.env.with_file_name("env").join("reader.env");
    assert!(std::fs::read_to_string(file)
        .is_ok_and(|text| text.contains("SONARR_KEY=") && text.contains("replacement-key")));
    assert!(machine.runner.seen().iter().any(
        |argv| argv.iter().any(|word| word == "up") && argv.iter().any(|word| word == "reader")
    ));
}

#[tokio::test]
async fn a_reader_that_is_not_running_takes_the_replacement_when_it_next_starts() {
    let (machine, installed) = crossed("crossed-stopped", Reporting::absent());

    let reached_now = reached(&machine, &installed, CROSSING).await;

    assert!(reached_now.iter().all(|one| one.reach
        == Reach::Pending {
            detail: format!("lemonfiber up --service {}", one.consumer)
        }));
    assert!(machine.runner.seen().is_empty());
}

#[tokio::test]
async fn a_reader_the_engine_would_not_recreate_is_named_as_not_reached() {
    let (mut refused, installed) = crossed("crossed-refused", running(&["reader"]));
    answering(
        &mut refused,
        Ok(Output {
            status: Some(1),
            stdout: String::new(),
            stderr: "no such image".to_owned(),
        }),
    );
    let (mut missing, _) = crossed("crossed-missing", running(&["reader"]));
    answering(
        &mut missing,
        Err(Failure::NotFound {
            program: "docker".to_owned(),
        }),
    );

    assert!(failed(
        &reached(&refused, &installed, CROSSING).await,
        "no such image"
    ));
    assert!(failed(
        &reached(&missing, &installed, CROSSING).await,
        "docker was not found"
    ));
}

#[tokio::test]
async fn a_secrets_file_that_cannot_be_written_is_named_and_nothing_restarts() {
    let (machine, installed) = crossed("crossed-unwritten", running(&["reader"]));
    unwritable_files(&machine);

    let reached_now = reached(&machine, &installed, CROSSING).await;

    assert_eq!(reached_now.len(), 2);
    assert!(reached_now
        .iter()
        .all(|one| matches!(one.reach, Reach::Failed { .. })));
    assert!(machine.runner.seen().is_empty());
}

#[tokio::test]
async fn a_rotation_reaches_nothing_where_no_plugin_reads_it_or_the_record_will_not_read() {
    let (machine, _) = crossed("crossed-untrusted", running(&["reader"]));
    assert!(crate::settings::run::crossed(&machine.ctx, CROSSING)
        .await
        .is_empty());
    let kept = crate::app::plugins::kept_at(&machine.ctx).unwrap_or_default();
    assert!(std::fs::write(kept, "{ not json").is_ok());
    assert!(crate::settings::run::crossed(&machine.ctx, CROSSING)
        .await
        .is_empty());
}

#[tokio::test]
async fn a_granted_secret_is_listed_as_granted_held_where_it_may_cross_and_withheld_otherwise() {
    let (machine, installed) = crossed("crossed-listed", running(&[]));
    let manifest = machine.ctx.stack.checked_manifest(machine.ctx.today()).ok();
    let readers = manifest
        .as_ref()
        .map(|manifest| crate::settings::readers::readers(manifest, &installed))
        .unwrap_or_default();
    let listed = |resolved: &Resolved| {
        super::super::listing::secrets(&machine.ctx, &readers, None, &installed, resolved)
            .into_iter()
            .find(|one| one.name == "reader:sonarr-key")
            .map(|one| (one.from, one.held))
    };

    let trusted_now = listed(&trusted(&machine, &installed));
    let untrusted = listed(&Resolved::of(&machine.ctx, manifest.as_ref(), &installed));

    assert_eq!(trusted_now, Some((Some(SecretFrom::Granted), true)));
    assert_eq!(untrusted, Some((Some(SecretFrom::Granted), false)));
}

#[tokio::test]
async fn a_change_with_nowhere_to_record_it_is_refused() {
    let mut machine = machine("nowhere", running(&[]));
    machine.ctx.settings.env_file = None;
    machine.ctx.settings.stack_dir = None;

    let refused = agreed(&machine.ctx, "domain", Some("home.example")).await;

    assert_eq!(code(&refused), Some("CONFIG-3"));
    assert!(crate::filed_stack(&machine.ctx).is_ok());
}

#[tokio::test]
async fn a_restart_over_a_stack_that_cannot_be_read_says_why_and_runs_nothing() {
    let machine = machine("unread-stack", running(&["komga"]));
    let mut unread = machine.ctx.clone();
    unread.stack = crate::test_support::nowhere();

    let said = super::super::restarting::restarted(&unread, &["komga".to_owned()]).await;

    assert!(said.is_some_and(|said| !said.is_empty()));
    assert!(machine.runner.seen().is_empty());
}

#[tokio::test]
async fn a_secret_no_installed_plugin_declares_is_read_from_the_store() {
    let (machine, _) = crossed("store-held", running(&[]));
    let mut readers = crate::settings::readers::Readers::default();
    readers.secrets.insert(
        "gluetun:wireguard-key".to_owned(),
        vec![crate::model::Reader {
            service: "gluetun".to_owned(),
            variable: "WIREGUARD_PRIVATE_KEY".to_owned(),
            default: None,
        }],
    );
    let resolved = Resolved::of(&machine.ctx, None, &[]);

    let listed = super::super::listing::secrets(&machine.ctx, &readers, None, &[], &resolved);

    assert!(listed
        .iter()
        .all(|one| one.name == "gluetun:wireguard-key" && !one.held && one.from.is_none()));
    assert_eq!(listed.len(), 1);
}

/// An assembled part's key of a curator is listed as the curator's, granted to it, rather
/// than as a value of the part's own held in the secret store.
#[tokio::test]
async fn an_assembled_parts_crossing_key_is_listed_as_granted() {
    let (machine, _) = crossed("assembled-granted", running(&[]));
    let manifest = lemonfiber_manifest::Manifest::from_toml(lemonfiber_bundled::STACK).ok();
    let readers = manifest
        .as_ref()
        .map(|manifest| crate::settings::readers::readers(manifest, &[]))
        .unwrap_or_default();
    let resolved = Resolved::of(&machine.ctx, manifest.as_ref(), &[]);

    let listed =
        super::super::listing::secrets(&machine.ctx, &readers, manifest.as_ref(), &[], &resolved);

    let granted = listed.iter().find(|one| one.name == "unpackerr:tv-key");
    assert_eq!(granted.map(|one| one.from), Some(Some(SecretFrom::Granted)));
}

/// A crossing key the operator sets by hand over the one recorded is written into every
/// file that reads it at once, as a rotation's is, so no reader keeps the value it
/// replaced.
#[tokio::test]
async fn a_crossing_key_set_by_hand_rewrites_every_file_that_reads_it() {
    let machine = machine("set-by-hand", Reporting::absent());
    registered(&machine, crate::test_support::a_trusted_crossing_reader());
    assert!(store::set(&machine.env, CROSSING, "the-replaced-key").is_ok());
    let file = machine.env.with_file_name("env").join("reader.env");
    assert!(store::write(&file, "SONARR_KEY=the-replaced-key\n").is_ok());

    let set = crate::app::configuring::set(
        &machine.ctx,
        crate::app::Setting::to(CROSSING, "set-by-hand"),
    )
    .await;

    assert!(set.is_ok(), "{set:?}");
    assert!(std::fs::read_to_string(&file).is_ok_and(
        |text| !text.contains("the-replaced-key") && text.contains("SONARR_KEY=set-by-hand")
    ));
}

/// A secret an undo puts back is written into every file that reads it, so no reader keeps
/// the value the undone change gave it.
#[tokio::test]
async fn a_secret_put_back_by_an_undo_reaches_every_file_that_reads_it() {
    let machine = machine("reheld", Reporting::absent());
    let mut holder = crate::test_support::a_placed("holder", &[], None, None);
    holder.fields.env.insert(
        "TOKEN".to_owned(),
        Value::Secret(SecretOf {
            secret: "token".to_owned(),
        }),
    );
    registered(
        &machine,
        crate::test_support::an_installed("komga", vec![holder]),
    );
    let secrets = crate::settings::secrets_at(&machine.ctx).unwrap_or_default();
    let keeping = [crate::settings::Keeping {
        name: "komga:token",
        value: "put-back",
        from: crate::settings::Source::Recipe,
        at: 1,
    }];
    assert!(crate::settings::kept(&secrets, &keeping, &|_| Ok(())).is_ok());
    let file = machine.env.with_file_name("env").join("holder.env");
    assert!(store::write(&file, "TOKEN=undone\n").is_ok());
    let carried = crate::app::recover::Carried {
        done: vec![crate::journal::Undo {
            target: "komga:token".to_owned(),
            action: crate::journal::Action::Rehold {
                name: "komga:token".to_owned(),
                earlier: None,
                kept: None,
                filed: None,
            },
        }],
        ..crate::app::recover::Carried::default()
    };

    let reheld = crate::app::recover::reheld(&machine.ctx, &carried);

    assert!(reheld.is_ok(), "{reheld:?}");
    assert!(std::fs::read_to_string(&file)
        .is_ok_and(|text| text.contains("TOKEN=put-back") && !text.contains("undone")));
}

/// A secret the operator enters by hand is written into the file that reads it at once,
/// and putting the change back writes the earlier one in again.
#[tokio::test]
async fn an_entered_secret_and_its_undo_each_reach_the_file_that_reads_it() {
    let machine = machine("entered", Reporting::absent());
    let mut holder = crate::test_support::a_placed("holder", &[], None, None);
    holder.fields.env.insert(
        "WG".to_owned(),
        Value::Secret(SecretOf {
            secret: "wireguard-key".to_owned(),
        }),
    );
    registered(
        &machine,
        crate::test_support::an_installed("gluetun", vec![holder]),
    );
    let file = machine.env.with_file_name("env").join("holder.env");
    let holds = |value: &str| {
        std::fs::read_to_string(&file).is_ok_and(|text| text == format!("WG={value}\n"))
    };
    let entered = |value: &'static str| {
        crate::app::configuring::set(
            &machine.ctx,
            crate::app::Setting::to(crate::credential::VPN_PRIVATE_KEY, value),
        )
    };

    let secrets = crate::settings::secrets_at(&machine.ctx).unwrap_or_default();
    let recipe = [crate::settings::Keeping {
        name: "gluetun:wireguard-key",
        value: "recipe-key",
        from: crate::settings::Source::Recipe,
        at: 1,
    }];
    assert!(crate::settings::kept(&secrets, &recipe, &|_| Ok(())).is_ok());

    assert!(entered("entered-key").await.is_ok());
    assert!(holds("entered-key"));
    let last =
        crate::app::recover::journal_at(&machine.env.with_file_name(crate::config::paths::JOURNAL))
            .ok()
            .and_then(|journal| journal.changes().last().map(|change| change.at.clone()))
            .unwrap_or_default();
    let undone = crate::app::putting_back::undo(&machine.ctx, Some(&last)).await;

    assert!(undone.is_ok(), "{undone:?}");
    assert!(holds("recipe-key"));
}

/// A crossing key an undo puts back is written into every file that reads it, so no reader
/// keeps the value the undone change gave it.
#[tokio::test]
async fn a_crossing_key_put_back_by_an_undo_reaches_every_file_that_reads_it() {
    let machine = machine("restored", Reporting::absent());
    registered(&machine, crate::test_support::a_trusted_crossing_reader());
    assert!(store::set(&machine.env, CROSSING, "put-back").is_ok());
    let file = machine.env.with_file_name("env").join("reader.env");
    assert!(store::write(&file, "SONARR_KEY=undone\n").is_ok());
    let carried = crate::app::recover::Carried {
        done: vec![crate::journal::Undo {
            target: CROSSING.to_owned(),
            action: crate::journal::Action::Restore {
                key: CROSSING.to_owned(),
                value: Some("put-back".to_owned()),
                wrote: "undone".to_owned(),
            },
        }],
        ..crate::app::recover::Carried::default()
    };

    let reheld = crate::app::recover::reheld(&machine.ctx, &carried);

    assert!(reheld.is_ok(), "{reheld:?}");
    assert!(std::fs::read_to_string(&file)
        .is_ok_and(|text| text.contains("SONARR_KEY=put-back") && !text.contains("undone")));
}

/// Where the files that read them live, under `machine`, made a file so none can be written.
pub(super) fn unwritable_files(machine: &Machine) {
    assert!(std::fs::write(machine.env.with_file_name("env"), "not a directory").is_ok());
}

/// Whether `refused` says a file reading the changed value could not be written, and that
/// the value changed all the same.
pub(super) fn refused_as_unreached(refused: Option<Box<crate::error::Problem>>) -> bool {
    refused.is_some_and(|problem| {
        problem.code == crate::error::codes::config::CONFIG_NOT_WRITTEN
            && problem.meaning == crate::settings::files::UNREACHED
    })
}

/// A crossing key set by hand that cannot reach a file that reads it is refused, saying
/// why, rather than reported set while a reader keeps the value it replaced.
#[tokio::test]
async fn a_crossing_key_set_by_hand_that_cannot_reach_its_reader_is_refused() {
    let machine = machine("set-unwritten", Reporting::absent());
    registered(&machine, crate::test_support::a_trusted_crossing_reader());
    assert!(store::set(&machine.env, CROSSING, "recorded").is_ok());
    unwritable_files(&machine);

    let set = crate::app::configuring::set(
        &machine.ctx,
        crate::app::Setting::to(CROSSING, "set-by-hand"),
    )
    .await;

    assert!(refused_as_unreached(set.err()));
}

/// A crossing key an undo puts back that cannot reach a file that reads it is refused,
/// saying why.
#[tokio::test]
async fn a_crossing_key_put_back_that_cannot_reach_its_reader_is_refused() {
    let machine = machine("restored-unwritten", Reporting::absent());
    registered(&machine, crate::test_support::a_trusted_crossing_reader());
    assert!(store::set(&machine.env, CROSSING, "put-back").is_ok());
    unwritable_files(&machine);
    let carried = crate::app::recover::Carried {
        done: vec![crate::journal::Undo {
            target: CROSSING.to_owned(),
            action: crate::journal::Action::Restore {
                key: CROSSING.to_owned(),
                value: Some("put-back".to_owned()),
                wrote: "undone".to_owned(),
            },
        }],
        ..crate::app::recover::Carried::default()
    };

    let reheld = crate::app::recover::reheld(&machine.ctx, &carried);

    assert!(refused_as_unreached(reheld.err()));
}
