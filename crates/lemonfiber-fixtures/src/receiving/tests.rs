use lemonfiber_ports::service::{Credential, Protocol};

use super::*;

/// A folder wanted at `path`.
fn folder(path: &str) -> RootFolder {
    RootFolder {
        path: path.to_owned(),
        media_type: "tv".to_owned(),
    }
}

/// A client wanted at `host`, filed under `tv`.
fn client(host: &str) -> DownloadClient {
    DownloadClient {
        name: "client".to_owned(),
        host: host.to_owned(),
        port: 8080,
        protocol: Protocol("usenet".to_owned()),
        credential: Credential::ApiKey("key".to_owned()),
        category: Category {
            field: "tvCategory".to_owned(),
            value: "tv".to_owned(),
        },
    }
}

/// A service answering normally registers what it is given under fresh ids and reads it
/// back, updates a client in place and names itself and one profile.
#[tokio::test]
async fn a_service_answering_normally_keeps_what_it_is_given() {
    let service = Receiving::with(Mode::Normal, Vec::new());

    assert!(service
        .register_root_folder(&folder("/data/tv/"))
        .await
        .is_ok());
    assert!(service
        .register_download_client(&client("box"))
        .await
        .is_ok());
    assert!(service
        .update_download_client("101", &client("box"))
        .await
        .is_ok());
    assert!(service
        .set_client_field("101", "tvCategory", Some("shows"))
        .await
        .is_ok());

    assert_eq!(
        service.registered().first().map(|held| held.path.clone()),
        Some("/data/tv".to_owned())
    );
    assert!(service
        .root_folders()
        .await
        .is_ok_and(|held| held.len() == 1));
    assert!(service.download_clients().await.is_ok_and(|held| held
        .first()
        .and_then(|one| one.category.as_ref())
        .is_some_and(|category| category.value == "shows")));
    assert!(service
        .identity()
        .await
        .is_ok_and(|named| named.name == "sonarr"));
    assert!(service
        .quality_profiles()
        .await
        .is_ok_and(|held| held.len() == 1));
    assert!(service.test_download_clients().await.is_err());
}

/// A service scripted with verdicts answers its client test with them.
#[tokio::test]
async fn a_probing_service_answers_its_client_test() {
    let probe = ClientProbe {
        id: "1".to_owned(),
        reachable: true,
        detail: None,
    };
    let service = Receiving::with_clients(Mode::Normal, Vec::new()).probing(vec![probe]);

    assert!(service
        .test_download_clients()
        .await
        .is_ok_and(|probes| probes.len() == 1));
}

/// A service that is down fails every write and read.
#[tokio::test]
async fn a_service_that_is_down_fails_everything() {
    let service = Receiving::with(Mode::Down, Vec::new());

    assert!(service
        .register_root_folder(&folder("/data/tv"))
        .await
        .is_err());
    assert!(service
        .register_download_client(&client("box"))
        .await
        .is_err());
    assert!(service
        .update_download_client("1", &client("box"))
        .await
        .is_err());
    assert!(service
        .set_client_field("1", "tvCategory", None)
        .await
        .is_err());
    assert!(service.root_folders().await.is_err());
    assert!(service.download_clients().await.is_err());
}

/// Each scripted refusal is the failure it names, and a swallowed write never appears.
#[tokio::test]
async fn each_scripted_refusal_is_its_own_failure() {
    let rejecting = Receiving::with(Mode::RejectsRegister, Vec::new());
    assert!(matches!(
        rejecting.register_root_folder(&folder("/data/tv")).await,
        Err(Failure::Refused { .. })
    ));
    assert!(matches!(
        rejecting.register_download_client(&client("box")).await,
        Err(Failure::Refused { .. })
    ));

    let swallowing = Receiving::with(Mode::Swallows, Vec::new());
    assert!(swallowing
        .register_root_folder(&folder("/data/tv"))
        .await
        .is_ok());
    assert!(swallowing
        .register_download_client(&client("box"))
        .await
        .is_ok());
    assert!(swallowing.registered().is_empty());

    let refusing_list = Receiving::with(Mode::RefusesList, Vec::new());
    assert!(matches!(
        refusing_list.root_folders().await,
        Err(Failure::Unauthorised { .. })
    ));
    assert!(matches!(
        refusing_list.download_clients().await,
        Err(Failure::Unauthorised { .. })
    ));

    assert!(matches!(
        Receiving::with(Mode::Unsupported, Vec::new())
            .root_folders()
            .await,
        Err(Failure::Unsupported { .. })
    ));
    assert!(matches!(
        Receiving::with(Mode::Disallowed, Vec::new())
            .root_folders()
            .await,
        Err(Failure::Disallowed { .. })
    ));
    assert!(matches!(
        Receiving::with(Mode::RefusesUpdate, Vec::new())
            .update_download_client("1", &client("box"))
            .await,
        Err(Failure::Refused { .. })
    ));
}

/// A service that drops after a registration answers its first read and fails the next.
#[tokio::test]
async fn a_service_dropping_after_a_registration_fails_the_read_back() {
    let folders = Receiving::with(Mode::DropsAfterRegister, Vec::new());
    assert!(folders.root_folders().await.is_ok());
    assert!(folders.root_folders().await.is_err());

    let clients = Receiving::with(Mode::DropsAfterRegister, Vec::new());
    assert!(clients.download_clients().await.is_ok());
    assert!(clients.download_clients().await.is_err());
}
