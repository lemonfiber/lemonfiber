use lemonfiber_ports::service::ApplicationKind;

use super::*;

/// An application reached at `base_url`.
fn app(base_url: &str) -> Application {
    Application {
        name: "curator".to_owned(),
        kind: ApplicationKind::Tv,
        indexer_url: "http://aggregator:9696".to_owned(),
        base_url: base_url.to_owned(),
        api_key: "key".to_owned(),
    }
}

/// One application it holds, at `base_url`.
fn held(base_url: &str) -> RegisteredApplication {
    RegisteredApplication {
        id: "1".to_owned(),
        base_url: base_url.to_owned(),
    }
}

/// An aggregator answering normally keeps what it is given, and a fresh key passes.
#[tokio::test]
async fn an_aggregator_answering_normally_keeps_what_it_is_given() {
    let aggregator = Applications::with(Mode::Normal, Vec::new());

    assert!(aggregator
        .register_application(&app("http://curator/"))
        .await
        .is_ok());
    assert!(aggregator.applications().await.is_ok_and(|held| held
        .first()
        .is_some_and(|one| one.base_url == "http://curator")));
    assert!(aggregator
        .test_application(&held("http://curator"))
        .await
        .is_ok());
}

/// A stale key fails its test until a new one is given, and a refused key is refused.
#[tokio::test]
async fn a_stale_key_fails_until_it_is_replaced() {
    let stale = Applications::stale(vec![held("http://curator")], false);
    assert!(stale
        .test_application(&held("http://curator"))
        .await
        .is_err());
    assert!(stale
        .rekey_application(&held("http://curator"), "new")
        .await
        .is_ok());
    assert!(stale
        .test_application(&held("http://curator"))
        .await
        .is_ok());
    assert_eq!(stale.rekeyed(), ["new"]);

    let refusing = Applications::stale(Vec::new(), false).refusing_rekey();
    assert!(matches!(
        refusing
            .rekey_application(&held("http://curator"), "new")
            .await,
        Err(Failure::Refused { .. })
    ));
    assert!(refusing.rekeyed().is_empty());
}

/// Each scripted mode fails as it names.
#[tokio::test]
async fn each_scripted_mode_fails_as_it_names() {
    let down = Applications::with(Mode::Down, Vec::new());
    assert!(down
        .register_application(&app("http://curator"))
        .await
        .is_err());
    assert!(down.applications().await.is_err());

    assert!(matches!(
        Applications::with(Mode::RejectsRegister, Vec::new())
            .register_application(&app("http://curator"))
            .await,
        Err(Failure::Refused { .. })
    ));
    let swallowing = Applications::with(Mode::Swallows, Vec::new());
    assert!(swallowing
        .register_application(&app("http://curator"))
        .await
        .is_ok());
    assert!(swallowing
        .applications()
        .await
        .is_ok_and(|held| held.is_empty()));

    assert!(matches!(
        Applications::with(Mode::RefusesList, Vec::new())
            .applications()
            .await,
        Err(Failure::Unauthorised { .. })
    ));
    let dropping = Applications::with(Mode::DropsAfterRegister, Vec::new());
    assert!(dropping.applications().await.is_ok());
    assert!(dropping.applications().await.is_err());
}
