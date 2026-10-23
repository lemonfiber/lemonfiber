use super::changing::{machine, running, Machine};
use crate::config::store;
use crate::journal::Kind;
use crate::plugin::{Grant, Installed, Register};
use crate::settings::Resolved;

const CROSSING: &str = "SONARR_API_KEY";

/// The grant of the series curator's key to the reader, for the capability it fills.
fn the_grant() -> Grant {
    Grant {
        credential: CROSSING.to_owned(),
        of: "sonarr".to_owned(),
        serves: "media.serve".to_owned(),
        at: "1".to_owned(),
    }
}

/// The reader, its crossing secret naming what it is for, and holding `granted`.
fn reader(granted: Vec<Grant>) -> Installed {
    let mut reader = crate::test_support::a_crossing_reader();
    for secret in &mut reader.declared.secrets {
        secret.serves = Some("media.serve".to_owned());
    }
    reader.granted = granted;
    reader
}

/// A machine holding the curator's key, with `installed` the one plugin recorded on it.
fn granting(name: &str, installed: Installed) -> Machine {
    let machine = machine(name, running(&["reader"]));
    assert!(store::set(&machine.env, CROSSING, "series-key").is_ok());
    let mut register = Register::empty();
    assert!(register.record(installed).is_ok());
    assert!(std::fs::write(
        machine.env.with_file_name(crate::config::paths::PLUGINS),
        serde_json::to_string(&register).unwrap_or_default()
    )
    .is_ok());
    machine
}

/// What the record on `machine` holds for the reader now.
fn recorded(machine: &Machine) -> Vec<Grant> {
    crate::app::plugins::read(&machine.ctx)
        .ok()
        .and_then(|register| register.holds("reader").map(|one| one.granted.clone()))
        .unwrap_or_default()
}

/// Every grant change the journal on `machine` holds, as whether each was given.
fn journalled(machine: &Machine) -> Vec<(String, bool)> {
    crate::app::targets::layout(&machine.ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .map(|journal| {
            journal
                .changes()
                .iter()
                .filter_map(|change| match &change.kind {
                    Kind::Granted { grant, given, .. } => Some((grant.credential.clone(), *given)),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_third_party_plugin_reads_another_services_credential_only_where_it_was_granted() {
    let machine = granting("granted-reads", reader(Vec::new()));
    let manifest = machine.ctx.stack.checked_manifest(machine.ctx.today()).ok();
    let holds = |installed: Installed| {
        let resolved = Resolved::of(
            &machine.ctx,
            manifest.as_ref(),
            std::slice::from_ref(&installed),
        );
        resolved.reading(|reading| (reading.crossing)("reader", "sonarr"))
    };

    assert_eq!(
        holds(reader(Vec::new())),
        Some(crate::plugin::Holding::Withheld)
    );
    assert_eq!(
        holds(reader(vec![the_grant()])),
        Some(crate::plugin::Holding::Crossing(CROSSING.to_owned()))
    );
}

#[test]
fn a_third_party_install_asks_one_grant_for_each_credential_it_names_and_a_trusted_one_none() {
    let machine = granting("granted-asked", reader(Vec::new()));
    let Ok(manifest) = machine.ctx.stack.checked_manifest(machine.ctx.today()) else {
        return;
    };
    let would = reader(Vec::new());

    let asked = crate::app::plugins::granting::asked(&machine.ctx, &manifest, &[], &would, &[]);
    let trusted = crate::app::plugins::granting::asked(
        &machine.ctx,
        &manifest,
        &[],
        &would,
        &crate::test_support::first_party("reader"),
    );

    assert_eq!(
        asked,
        vec![Grant {
            at: String::new(),
            ..the_grant()
        }]
    );
    assert!(trusted.is_empty());
    assert_eq!(
        crate::plugin::asked(
            &Installed {
                granted: asked,
                ..would
            },
            &[]
        ),
        vec![format!("{CROSSING}@reader")]
    );
}

#[test]
fn a_third_party_secret_of_another_service_naming_nothing_it_is_for_is_refused() {
    let mut unaimed = reader(Vec::new());
    for secret in &mut unaimed.declared.secrets {
        secret.serves = None;
    }

    let refused = crate::app::plugins::granting::aimed(&unaimed, &[]);
    let trusted =
        crate::app::plugins::granting::aimed(&unaimed, &crate::test_support::first_party("reader"));

    assert_eq!(
        refused
            .err()
            .map(|problem| problem.code.as_str().to_owned()),
        Some("PLUGIN-46".to_owned())
    );
    assert!(trusted.is_ok());
    assert!(crate::app::plugins::granting::aimed(&reader(Vec::new()), &[]).is_ok());
}

#[tokio::test]
async fn a_revoke_is_read_first_then_ends_the_grant_rewrites_and_restarts_what_read_it() {
    let machine = granting("granted-revoked", reader(vec![the_grant()]));

    let read = crate::app::plugins::granting::revoke(
        &machine.ctx,
        "reader",
        &CROSSING.to_lowercase(),
        None,
    )
    .await;
    assert!(read.is_ok(), "{read:?}");
    let Ok(read) = read else {
        return;
    };
    assert!(!read.ended);
    assert_eq!(read.restarts, vec!["reader", "reader-worker"]);
    assert_eq!(recorded(&machine), vec![the_grant()]);

    let ended = crate::app::plugins::granting::revoke(
        &machine.ctx,
        "reader",
        CROSSING,
        Some(&read.agreement),
    )
    .await;

    assert!(ended.as_ref().is_ok_and(|ended| ended.ended), "{ended:?}");
    assert!(recorded(&machine).is_empty());
    assert_eq!(journalled(&machine), vec![(CROSSING.to_owned(), false)]);
    let file = machine.env.with_file_name("env").join("reader.env");
    assert!(std::fs::read_to_string(file).is_ok_and(|text| !text.contains("series-key")));
    assert!(machine.runner.seen().iter().any(
        |argv| argv.iter().any(|word| word == "up") && argv.iter().any(|word| word == "reader")
    ));
}

#[tokio::test]
async fn a_revoke_of_a_grant_not_held_or_over_a_moved_offer_is_refused_and_ends_nothing() {
    let machine = granting("granted-unheld", reader(vec![the_grant()]));

    let unheld =
        crate::app::plugins::granting::revoke(&machine.ctx, "reader", "RADARR_API_KEY", None).await;
    let moved =
        crate::app::plugins::granting::revoke(&machine.ctx, "reader", CROSSING, Some("moved"))
            .await;

    let unheld = unheld.err();
    assert_eq!(
        unheld.as_ref().map(|problem| problem.code.as_str()),
        Some("PLUGIN-47")
    );
    assert!(unheld
        .and_then(|problem| problem.detail)
        .is_some_and(|detail| detail.contains(CROSSING)));
    assert!(moved.is_err());
    assert_eq!(recorded(&machine), vec![the_grant()]);
    assert!(journalled(&machine).is_empty());
}

#[test]
fn an_undone_grant_gives_back_only_what_is_absent_and_ends_only_the_approval_it_names() {
    let mut register = Register::empty();
    assert!(register.record(reader(Vec::new())).is_ok());
    let later = Grant {
        at: "2".to_owned(),
        ..the_grant()
    };

    assert!(register.regrant("reader", &the_grant(), true));
    assert!(!register.regrant("reader", &later, true));
    assert!(!register.regrant("reader", &later, false));
    assert!(!register.regrant("elsewhere", &the_grant(), true));
    assert!(register.regrant("reader", &the_grant(), false));
    assert!(register
        .holds("reader")
        .is_some_and(|one| one.granted.is_empty()));
}

#[test]
fn a_grant_is_journalled_as_ended_where_an_act_drops_it_and_given_where_it_adds_one() {
    let given = crate::app::plugins::granting::journalled;
    let machine = granting("granted-journalled", reader(Vec::new()));
    let later = Grant {
        at: "2".to_owned(),
        ..the_grant()
    };

    assert!(given(&machine.ctx, "reader", &[], &[], "3").is_ok());
    assert!(given(&machine.ctx, "reader", &[the_grant()], &[later], "3").is_ok());

    assert_eq!(
        journalled(&machine),
        vec![(CROSSING.to_owned(), false), (CROSSING.to_owned(), true)]
    );
}
