//! A phone's word that a wake arrived, asked through the dispatcher, and each phone as the
//! account of where alerts go reads it.

use super::{alert, machine, refused, registering};
use crate::alert::{Alert, Moment};
use crate::config::Settings;
use crate::error::codes::notify::NO_SUCH_WAKE;
use crate::model::{BackendKind, BackendProof, ChosenBy};
use crate::push::run::{asked, said_as, Asked};

fn woken(id: &str) -> Asked {
    Asked::Woken {
        name: "pixel".to_owned(),
        id: id.to_owned(),
        owner: None,
    }
}

#[test]
fn a_wake_nobody_sent_is_refused_and_its_id_is_never_printed() {
    let (ctx, _distributor) = machine("push-woken-asked", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());

    assert_eq!(refused(&ctx, woken("the-wake-id")), Some(NO_SUCH_WAKE));
    let said = format!("{:?}", woken("the-wake-id"));
    assert!(said.contains("pixel") && !said.contains("the-wake-id"));
}

#[test]
fn an_alert_with_nothing_naming_it_is_said_as_nothing() {
    let unnamed = Alert {
        id: None,
        ..alert("service.sonarr", Moment::Onset)
    };

    assert_eq!(said_as(&unnamed), None);
    assert!(said_as(&alert("service.sonarr", Moment::Onset)).is_some());
}

#[test]
fn a_registration_the_journal_has_nowhere_to_go_is_refused_and_kept_nowhere() {
    let dir = lemonfiber_fixtures::scratch::Scratch::named("push-unjournalled-register").kept();
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(dir.join("config"));
    let ctx = crate::test_support::a_context()
        .settings(Settings {
            env_file: Some(dir.join("config").join(".env")),
            ..Settings::default()
        })
        .build();

    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_err());
    assert!(crate::push::telling::as_backends(&ctx).is_empty());
}

#[test]
fn every_phone_is_listed_where_alerts_go_by_its_distributors_host() {
    let (ctx, _distributor) = machine("push-as-backends", 201);
    assert!(asked(&ctx, registering("pixel", "https://push.example/a")).is_ok());

    let listed: Vec<_> = crate::notify::reading::report(&ctx)
        .map(|report| report.backends)
        .unwrap_or_default()
        .into_iter()
        .map(|one| (one.id, one.kind, one.reaches, one.chosen_by, one.proof))
        .collect();

    assert_eq!(
        listed,
        [(
            "pixel".to_owned(),
            BackendKind::Unifiedpush,
            "push.example".to_owned(),
            ChosenBy::Operator,
            BackendProof::Unproven
        )]
    );
}
