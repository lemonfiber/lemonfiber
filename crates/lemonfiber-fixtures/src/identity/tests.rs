use lemonfiber_ports::service::{Protocol, QualityProfile};

use super::*;

/// Where a target or a media server is reached.
fn at() -> Endpoint {
    Endpoint {
        host: "curator".to_owned(),
        port: 8989,
        base: String::new(),
    }
}

/// A media server answers its wizard and its account creation as scripted, and takes a
/// replaced password.
#[tokio::test]
async fn a_media_server_answers_its_setup_as_scripted() {
    let fresh = MediaSetup::new(Startup::Fresh, Create::Ok);
    assert!(fresh.startup_completed().await.is_ok_and(|done| !done));
    assert!(fresh.create_admin("admin", "pw").await.is_ok());
    assert!(fresh.replace_admin_password("pw").await.is_ok());

    let completed = MediaSetup::new(Startup::Completed, Create::Rejects);
    assert!(completed.startup_completed().await.is_ok_and(|done| done));
    assert!(matches!(
        completed.create_admin("admin", "pw").await,
        Err(Failure::Refused { .. })
    ));
    assert!(MediaSetup::new(Startup::Down, Create::Ok)
        .startup_completed()
        .await
        .is_err());
}

/// A request service reads as its gate first and its read-back after, configures and
/// establishes as scripted, and keeps what it is told to tell the household.
#[tokio::test]
async fn a_request_service_answers_its_setup_as_scripted() {
    let source = IdentitySource {
        at: "http://media:8096".to_owned(),
        protocol: Protocol("media".to_owned()),
        credential: None,
    };
    let service = RequestSetup::new(Init::Fresh, Init::Done, Configure::Ok);
    assert!(service.initialized().await.is_ok_and(|done| !done));
    assert!(service.initialized().await.is_ok_and(|done| done));
    assert!(service.establish(&source).await.is_ok());
    assert!(service.configure_identity(&source).await.is_ok());
    let telling = Telling {
        enabled: true,
        ..Telling::default()
    };
    assert!(service.tell(&telling).await.is_ok());
    assert!(service.telling().await.is_ok_and(|told| told == telling));

    let refusing =
        RequestSetup::new(Init::Done, Init::Done, Configure::Rejects).refusing_an_owner();
    assert!(matches!(
        refusing.establish(&source).await,
        Err(Failure::Refused { detail, .. }) if detail == NO_OWNER
    ));
    assert!(refusing.configure_identity(&source).await.is_err());

    let down = RequestSetup::new(Init::Down, Init::Down, Configure::Ok);
    assert!(down.initialized().await.is_err());
    assert!(down.telling().await.is_err());
    assert!(down.tell(&telling).await.is_err());
}

/// Everything a request service holds beyond its setup is empty, and every write to it
/// is taken.
#[tokio::test]
async fn a_request_service_holds_nothing_beyond_its_setup() {
    let service = RequestSetup::new(Init::Done, Init::Done, Configure::Ok);
    let target = RegisteredTarget {
        id: "1".to_owned(),
        at: at(),
        key: "key".to_owned(),
        kind: Kind::Tv,
    };
    let wanted = FulfilmentTarget {
        name: "curator".to_owned(),
        at: at(),
        moved_from: None,
        key: "key".to_owned(),
        kind: Kind::Tv,
        profile: QualityProfile {
            id: 1,
            name: "Any".to_owned(),
        },
        folder: "/data/tv".to_owned(),
    };

    assert!(service.answers().await.is_ok());
    assert!(service.requests().await.is_ok_and(|asked| asked.is_empty()));
    assert!(service
        .fulfilment_targets()
        .await
        .is_ok_and(|held| held.is_empty()));
    assert!(service.add_fulfilment_target(&wanted).await.is_ok());
    assert!(service
        .move_fulfilment_target(&target, &at(), "key")
        .await
        .is_ok());
    assert!(service.minted_as().await.is_ok_and(|name| name == "Seerr"));
    assert!(service
        .test_fulfilment_target(Kind::Movies, &at(), "key")
        .await
        .is_ok());
    assert!(service.media_server_link().await.is_err());
    assert!(service.link_media_server(&at(), "key").await.is_ok());
    assert!(service
        .member_for("m")
        .await
        .is_ok_and(|member| member.is_none()));
    assert!(service.requesting("m").await.is_ok_and(|may| may.is_none()));
    assert!(service.approval_first("m").await.is_ok());
    assert!(service.remove_member("m").await.is_ok());
    assert!(service.link_members(&["m".to_owned()]).await.is_ok());
}
