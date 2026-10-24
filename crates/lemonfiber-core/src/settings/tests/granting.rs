use super::changing::{machine, running, Machine};
use super::reaching::{refused_as_unreached, unwritable_files};
use crate::config::store;
use crate::journal::Kind;
use crate::plugin::{Grant, Installed, Register};
use crate::settings::Resolved;
use crate::test_support::a_grant as the_grant;
use lemonfiber_fixtures::support::Reporting;

const CROSSING: &str = "SONARR_API_KEY";

/// The reader, its crossing secret naming what it is for, and holding `granted`.
fn reader(granted: Vec<Grant>) -> Installed {
    let mut reader = crate::test_support::a_crossing_reader();
    for secret in &mut reader.declared.secrets {
        secret.serves = Some("media.serve".to_owned());
    }
    reader.granted = granted;
    reader
}

/// A machine holding the curator's key, with `installed` the one plugin recorded on it
/// and the reader running.
fn granting(name: &str, installed: Installed) -> Machine {
    granting_on(name, installed, running(&["reader"]))
}

/// A machine holding the curator's key, with `installed` the one plugin recorded on it and
/// the engine reporting `engine`.
fn granting_on(name: &str, installed: Installed, engine: Reporting) -> Machine {
    let machine = machine(name, engine);
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
    let file = machine.env.with_file_name("env").join("reader.env");
    assert!(store::write(&file, "SONARR_KEY=series-key\n").is_ok());

    let read = crate::app::plugins::granting::revoke(
        &machine.ctx,
        "reader",
        &CROSSING.to_lowercase(),
        None,
    )
    .await;
    assert!(read.is_ok(), "{read:?}");
    let Ok((read, unchanged)) = read else {
        return;
    };
    assert!(!read.ended);
    assert!(unchanged
        .holds("reader")
        .is_some_and(|one| one.granted == vec![the_grant()]));
    assert_eq!(read.restarts, vec!["reader", "reader-worker"]);
    assert_eq!(recorded(&machine), vec![the_grant()]);

    let ended = crate::app::plugins::granting::revoke(
        &machine.ctx,
        "reader",
        CROSSING,
        Some(&read.agreement),
    )
    .await;

    assert!(
        ended.as_ref().is_ok_and(|(ended, after)| ended.ended
            && after
                .holds("reader")
                .is_some_and(|one| one.granted.is_empty())),
        "{ended:?}"
    );
    assert!(recorded(&machine).is_empty());
    assert_eq!(journalled(&machine), vec![(CROSSING.to_owned(), false)]);
    assert!(std::fs::read_to_string(file).is_ok_and(|text| !text.contains("series-key")));
    assert!(machine.runner.seen().iter().any(
        |argv| argv.iter().any(|word| word == "up") && argv.iter().any(|word| word == "reader")
    ));
}

#[tokio::test]
async fn a_revoke_writes_the_credential_out_of_a_reader_that_is_not_running() {
    let machine = granting_on(
        "granted-revoked-stopped",
        reader(vec![the_grant()]),
        Reporting::absent(),
    );
    let file = machine.env.with_file_name("env").join("reader.env");
    assert!(store::write(&file, "SONARR_KEY=series-key\n").is_ok());
    let read = crate::app::plugins::granting::revoke(&machine.ctx, "reader", CROSSING, None).await;
    let agreement = read.map(|(read, _)| read.agreement).unwrap_or_default();

    let ended =
        crate::app::plugins::granting::revoke(&machine.ctx, "reader", CROSSING, Some(&agreement))
            .await;

    assert!(ended.is_ok_and(|(ended, _)| ended.ended && ended.unrestarted.is_none()));
    assert!(std::fs::read_to_string(file).is_ok_and(|text| !text.contains("series-key")));
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
    let elsewhere =
        crate::app::plugins::granting::revoke(&machine.ctx, "elsewhere", CROSSING, None).await;
    assert!(elsewhere
        .err()
        .and_then(|problem| problem.detail)
        .is_some_and(|detail| detail == "it holds none"));
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

#[test]
fn a_grant_is_asked_once_for_each_credential_held_here_and_never_for_an_aimless_secret() {
    let machine = granting("granted-asked-once", reader(Vec::new()));
    let Ok(manifest) = machine.ctx.stack.checked_manifest(machine.ctx.today()) else {
        return;
    };
    let mut would = reader(Vec::new());
    let first = would.declared.secrets.first().cloned();
    for (id, of, serves) in [
        ("sonarr-again", "sonarr", Some("media.read")),
        ("sonarr-aimless", "sonarr", None),
        ("nowhere-key", "nowhere", Some("media.find")),
    ] {
        if let Some(mut secret) = first.clone() {
            id.clone_into(&mut secret.id);
            of.clone_into(&mut secret.of);
            secret.serves = serves.map(str::to_owned);
            would.declared.secrets.push(secret);
        }
    }

    let asked = crate::app::plugins::granting::asked(&machine.ctx, &manifest, &[], &would, &[]);

    assert_eq!(
        asked,
        vec![Grant {
            at: String::new(),
            ..the_grant()
        }]
    );
}

#[test]
fn an_approved_grant_is_stamped_when_it_was_approved() {
    let asked = Grant {
        at: String::new(),
        ..the_grant()
    };

    assert_eq!(
        crate::app::plugins::granting::approved(&[asked], "1"),
        vec![the_grant()]
    );
}

#[test]
fn a_grant_change_with_no_journal_to_take_it_is_refused() {
    let given = crate::app::plugins::granting::journalled;
    let homeless = crate::test_support::a_context().build();
    let machine = granting("granted-unjournalled", reader(Vec::new()));
    let journal = crate::app::targets::layout(&machine.ctx).map(|paths| paths.journal());
    assert!(journal.is_some_and(|journal| std::fs::create_dir_all(journal).is_ok()));

    assert!(given(&homeless, "reader", &[], &[the_grant()], "3").is_err());
    assert!(given(&machine.ctx, "reader", &[], &[the_grant()], "3").is_err());
}

#[tokio::test]
async fn a_revoke_whose_reader_cannot_be_rewritten_ends_the_grant_and_says_so() {
    let machine = granting("granted-unreached", reader(vec![the_grant()]));
    let read = crate::app::plugins::granting::revoke(&machine.ctx, "reader", CROSSING, None).await;
    let agreement = read.map(|(read, _)| read.agreement).unwrap_or_default();
    unwritable_files(&machine);

    let refused =
        crate::app::plugins::granting::revoke(&machine.ctx, "reader", CROSSING, Some(&agreement))
            .await;

    assert!(refused_as_unreached(refused.err()));
    assert!(recorded(&machine).is_empty());
}

#[tokio::test]
async fn a_revoke_asked_of_the_credentials_answers_with_every_grant_as_it_then_stands() {
    let machine = granting("granted-inventoried", reader(vec![the_grant()]));
    let asked = |agreement: Option<String>| crate::app::Asking::Revoke {
        plugin: "reader".to_owned(),
        credential: CROSSING.to_owned(),
        agreement,
    };

    let read = crate::app::credentials::credentials(&machine.ctx, asked(None)).await;
    let agreement = read
        .as_ref()
        .ok()
        .and_then(|read| read.revoked.as_ref())
        .map(|revoked| revoked.agreement.clone());
    let ended = crate::app::credentials::credentials(&machine.ctx, asked(agreement)).await;

    assert!(
        read.as_ref().is_ok_and(|read| read.grants
            == vec![crate::credential::Granted::of("reader", &the_grant())]
            && read.revoked.as_ref().is_some_and(|revoked| !revoked.ended)),
        "{read:?}"
    );
    assert!(
        ended.as_ref().is_ok_and(|ended| ended.grants.is_empty()
            && ended.revoked.as_ref().is_some_and(|revoked| revoked.ended)),
        "{ended:?}"
    );
}

#[tokio::test]
async fn a_revoke_asked_of_the_credentials_for_a_grant_not_held_is_refused_and_ends_nothing() {
    let machine = granting("granted-inventoried-unheld", reader(vec![the_grant()]));

    let refused = crate::app::credentials::credentials(
        &machine.ctx,
        crate::app::Asking::Revoke {
            plugin: "reader".to_owned(),
            credential: "RADARR_API_KEY".to_owned(),
            agreement: None,
        },
    )
    .await;

    assert!(refused.is_err_and(|problem| problem.code == crate::error::codes::plugin::NO_GRANT));
    assert_eq!(recorded(&machine), vec![the_grant()]);
}

/// The stamp of the newest change the journal on `machine` holds.
fn newest(machine: &Machine) -> String {
    crate::app::targets::layout(&machine.ctx)
        .and_then(|paths| crate::app::recover::journal_at(&paths.journal()).ok())
        .and_then(|journal| journal.changes().last().map(|change| change.at.clone()))
        .unwrap_or_default()
}

/// The reader's secrets file on `machine`, holding the curator's key as it was handed over.
fn handed(machine: &Machine) -> std::path::PathBuf {
    let file = machine.env.with_file_name("env").join("reader.env");
    assert!(store::write(&file, "SONARR_KEY=series-key\n").is_ok());
    file
}

#[tokio::test]
async fn an_undone_revoke_gives_the_grant_back_and_it_crosses_again() {
    let machine = granting_on(
        "granted-undone",
        reader(vec![the_grant()]),
        Reporting::absent(),
    );
    let file = handed(&machine);
    let read = crate::app::plugins::granting::revoke(&machine.ctx, "reader", CROSSING, None).await;
    let agreement = read.map(|(read, _)| read.agreement).unwrap_or_default();
    let ended =
        crate::app::plugins::granting::revoke(&machine.ctx, "reader", CROSSING, Some(&agreement))
            .await;
    assert!(ended.is_ok_and(|(ended, _)| ended.ended));
    assert!(std::fs::read_to_string(&file).is_ok_and(|text| !text.contains("series-key")));

    let undone = crate::app::putting_back::undo(&machine.ctx, Some(&newest(&machine))).await;

    assert!(undone.is_ok(), "{undone:?}");
    assert_eq!(recorded(&machine), vec![the_grant()]);
    assert!(std::fs::read_to_string(&file).is_ok_and(|text| text.contains("series-key")));
    assert_eq!(
        journalled(&machine),
        vec![(CROSSING.to_owned(), false), (CROSSING.to_owned(), true)]
    );
}

#[tokio::test]
async fn an_undone_grant_ends_it_and_writes_the_credential_out() {
    let machine = granting_on(
        "granted-given-undone",
        reader(vec![the_grant()]),
        Reporting::absent(),
    );
    let file = handed(&machine);
    assert!(crate::app::plugins::granting::journalled(
        &machine.ctx,
        "reader",
        &[],
        &[the_grant()],
        "2"
    )
    .is_ok());

    let undone = crate::app::putting_back::undo(&machine.ctx, Some("2")).await;

    assert!(undone.is_ok(), "{undone:?}");
    assert!(recorded(&machine).is_empty());
    assert!(std::fs::read_to_string(&file).is_ok_and(|text| !text.contains("series-key")));
}

#[test]
fn a_grant_given_back_that_is_already_held_is_left_as_it_stands() {
    let machine = granting("granted-held", reader(vec![the_grant()]));
    let record = machine.env.with_file_name(crate::config::paths::PLUGINS);
    let before = std::fs::read_to_string(&record).unwrap_or_default();
    let regrant = crate::journal::Undo {
        target: CROSSING.to_owned(),
        action: crate::journal::Action::Regrant {
            plugin: "reader".to_owned(),
            grant: the_grant(),
            give: true,
        },
    };

    let carried = crate::app::recover::carrying_out(
        &lemonfiber_adapters::Disk,
        &[regrant],
        &machine.env,
        Vec::new(),
    );

    assert_eq!(carried.map(|carried| carried.done.len()).ok(), Some(1));
    assert_eq!(std::fs::read_to_string(&record).unwrap_or_default(), before);
}

#[test]
fn every_grant_a_record_holds_is_kept_in_the_order_of_its_credentials() {
    let mut register = Register::empty();
    assert!(register.record(reader(Vec::new())).is_ok());
    let film = Grant {
        credential: "RADARR_API_KEY".to_owned(),
        of: "radarr".to_owned(),
        ..the_grant()
    };

    assert!(register.regrant("reader", &the_grant(), true));
    assert!(register.regrant("reader", &film, true));

    let held: Vec<String> = register
        .holds("reader")
        .map(|one| {
            one.granted
                .iter()
                .map(|grant| grant.credential.clone())
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(held, vec!["RADARR_API_KEY", CROSSING]);
}
