//! Root folders and download clients written to a service.

use crate::{sabnzbd, sonarr};
use lemonfiber_core::ports::http::Method;
use lemonfiber_core::ports::service::{
    Category, ClientKind, Credential, DownloadClient, Failure, RootFolder,
};
use lemonfiber_core::servarr::Servarr;
use lemonfiber_fixtures::http::{Answer, Fake};

#[tokio::test]
async fn a_root_folder_is_posted_to_its_endpoint() {
    let fake = Fake::always(Answer::reply(201, ""));
    let folder = RootFolder {
        path: "/data/media/tv".to_owned(),
        media_type: "tv".to_owned(),
    };
    assert!(sonarr(&fake).register_root_folder(&folder).await.is_ok());

    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.url.ends_with("/api/v3/rootfolder")));
    assert!(sent.is_some_and(|request| request
        .body
        .is_some_and(|body| body.contains("/data/media/tv"))));
}

/// A `qBittorrent` download client: a torrent client authenticated by a login.
fn qbit() -> DownloadClient {
    DownloadClient {
        name: "qBittorrent".to_owned(),
        host: "qbittorrent".to_owned(),
        port: 8081,
        kind: ClientKind::Qbittorrent,
        credential: Credential::UserPass {
            username: "admin".to_owned(),
            password: "web-pass".to_owned(),
        },
        category: Category {
            field: "movieCategory".to_owned(),
            value: "movies".to_owned(),
        },
    }
}

#[tokio::test]
async fn a_sabnzbd_client_is_posted_as_its_usenet_implementation() {
    let fake = Fake::always(Answer::reply(201, ""));
    assert!(sonarr(&fake)
        .register_download_client(&sabnzbd())
        .await
        .is_ok());

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    for expected in [
        r#""implementation":"Sabnzbd""#,
        r#""configContract":"SabnzbdSettings""#,
        r#""protocol":"usenet""#,
        r#""name":"apiKey""#,
        "sab-key",
        r#""name":"tvCategory""#,
    ] {
        assert!(
            body.contains(expected),
            "SABnzbd body missing {expected}: {body}"
        );
    }
    // A Usenet client is not told a username and password.
    assert!(!body.contains(r#""name":"username""#));
}

#[tokio::test]
async fn a_qbittorrent_client_is_posted_as_its_torrent_implementation() {
    let fake = Fake::always(Answer::reply(201, ""));
    assert!(sonarr(&fake)
        .register_download_client(&qbit())
        .await
        .is_ok());

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    for expected in [
        r#""implementation":"QBittorrent""#,
        r#""configContract":"QBittorrentSettings""#,
        r#""protocol":"torrent""#,
        r#""name":"username""#,
        r#""name":"password""#,
        "web-pass",
        r#""name":"movieCategory""#,
    ] {
        assert!(
            body.contains(expected),
            "qBittorrent body missing {expected}: {body}"
        );
    }
    // A torrent client authenticated by login is not told an API key.
    assert!(!body.contains(r#""name":"apiKey""#));
}

#[tokio::test]
async fn a_download_client_is_posted_to_its_endpoint() {
    let fake = Fake::always(Answer::reply(201, ""));
    assert!(sonarr(&fake)
        .register_download_client(&sabnzbd())
        .await
        .is_ok());

    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.url.ends_with("/api/v3/downloadclient")));
    assert!(sent.is_some_and(|request| request.body.is_some_and(|body| body.contains("SABnzbd"))));
}

#[tokio::test]
async fn an_updated_download_client_is_put_to_its_id_carrying_it() {
    let fake = Fake::always(Answer::reply(200, ""));
    assert!(sonarr(&fake)
        .update_download_client("7", &sabnzbd())
        .await
        .is_ok());

    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.method == Method::Put
            && request.url.ends_with("/api/v3/downloadclient/7")));
    // The document rewrites the one that is there rather than adding a second: it names
    // the id the service assigned.
    assert!(sent.is_some_and(|request| request.body.is_some_and(|body| body.contains(r#""id":7"#))));
}

/// Putting one field back reads the client, changes that field, and writes the rest of the
/// document exactly as the service gave it — which is what lets a reversal restore a
/// category without ever having held the client's credential.
#[tokio::test]
async fn one_field_is_put_back_leaving_the_rest_of_the_client_alone() {
    let held = r#"{"id":7,"name":"SABnzbd","fields":[{"name":"host","value":"sabnzbd"},
        {"name":"apiKey","value":"kept"},{"name":"tvCategory","value":"mine"}]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, held),
        Answer::reply(200, String::new()),
    ]);

    assert!(sonarr(&fake)
        .set_client_field("7", "tvCategory", Some("tv-sonarr"))
        .await
        .is_ok());

    let sent = fake.request();
    assert!(sent
        .as_ref()
        .is_some_and(|request| request.method == Method::Put
            && request.url.ends_with("/api/v3/downloadclient/7")));
    let body = sent.and_then(|request| request.body).unwrap_or_default();
    assert!(body.contains("tv-sonarr"), "{body}");
    // Everything the reversal never knew about goes back untouched, credential included.
    assert!(body.contains("kept"), "{body}");
    assert!(!body.contains("mine"), "{body}");
}

/// A field that held nothing before is taken out rather than set to the empty string,
/// which a service would read as a value somebody chose.
#[tokio::test]
async fn a_field_put_back_to_nothing_is_taken_out() {
    let held = r#"{"id":7,"fields":[{"name":"host","value":"sabnzbd"},
        {"name":"tvCategory","value":"mine"}]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, held),
        Answer::reply(200, String::new()),
    ]);

    assert!(sonarr(&fake)
        .set_client_field("7", "tvCategory", None)
        .await
        .is_ok());

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(!body.contains("tvCategory"), "{body}");
}

/// A field the client does not carry is added, so a reversal can put back a category the
/// service dropped rather than reporting success and changing nothing.
#[tokio::test]
async fn a_field_the_client_does_not_carry_is_added() {
    let held = r#"{"id":7,"fields":[{"name":"host","value":"sabnzbd"}]}"#;
    let fake = Fake::in_turn(vec![
        Answer::reply(200, held),
        Answer::reply(200, String::new()),
    ]);

    assert!(sonarr(&fake)
        .set_client_field("7", "tvCategory", Some("tv-sonarr"))
        .await
        .is_ok());

    let body = fake
        .request()
        .and_then(|request| request.body)
        .unwrap_or_default();
    assert!(body.contains("tv-sonarr"), "{body}");
}

/// A client whose document carries no settings at all is refused rather than written to on
/// a guess about a shape this build does not recognise.
#[tokio::test]
async fn a_client_with_no_settings_to_put_back_is_refused() {
    let fake = Fake::always(Answer::reply(200, r#"{"id":7}"#));

    assert!(matches!(
        sonarr(&fake)
            .set_client_field("7", "tvCategory", None)
            .await,
        Err(Failure::Refused { .. })
    ));
}

/// A service that will not hand the client over cannot have one field put back, and says
/// so rather than writing a document it never read.
#[tokio::test]
async fn a_client_that_cannot_be_read_is_not_written_back() {
    let fake = Fake::always(Answer::reply(500, "boom"));

    assert!(sonarr(&fake)
        .set_client_field("7", "tvCategory", None)
        .await
        .is_err());
}

#[tokio::test]
async fn an_update_with_an_id_the_service_did_not_assign_is_refused() {
    // A non-numeric id is not one a Servarr service assigns, so there is nothing to
    // address — refused rather than a malformed request sent.
    let fake = Fake::always(Answer::reply(200, ""));
    assert!(matches!(
        sonarr(&fake)
            .update_download_client("not-a-number", &sabnzbd())
            .await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_rejected_download_client_update_is_refused() {
    let fake = Fake::always(Answer::reply(400, "cannot update"));
    assert!(matches!(
        sonarr(&fake).update_download_client("7", &sabnzbd()).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn testing_download_clients_reads_each_services_verdict() {
    // `testall` answers one result per client: the valid one is reachable with nothing
    // to say, the one that failed carries the service's joined words, and one that
    // failed without words carries none — never an error, since a failing client is
    // the answer wanted, not a fault.
    let body = r#"[
        {"id":1,"isValid":true,"validationFailures":[]},
        {"id":2,"isValid":false,"validationFailures":[
            {"errorMessage":"unable to connect"},{"errorMessage":"timed out"}]},
        {"id":3,"isValid":false,"validationFailures":[]}
    ]"#;
    let fake = Fake::always(Answer::reply(200, body));
    let probes = sonarr(&fake)
        .test_download_clients()
        .await
        .unwrap_or_default();

    // The test is asked of the service's own `testall`.
    assert!(fake
        .request()
        .is_some_and(|request| request.method == Method::Post
            && request.url.ends_with("/api/v3/downloadclient/testall")));

    let reachable = probes.iter().find(|probe| probe.id == "1");
    assert!(reachable.is_some_and(|probe| probe.reachable && probe.detail.is_none()));
    let refused = probes.iter().find(|probe| probe.id == "2");
    assert!(refused.is_some_and(|probe| !probe.reachable
        && probe
            .detail
            .as_deref()
            .is_some_and(|detail| detail == "unable to connect; timed out")));
    let wordless = probes.iter().find(|probe| probe.id == "3");
    assert!(wordless.is_some_and(|probe| !probe.reachable && probe.detail.is_none()));
}

#[tokio::test]
async fn a_rejected_root_folder_registration_is_refused() {
    let fake = Fake::always(Answer::reply(400, "path already used"));
    let folder = RootFolder {
        path: "/data/media/tv".to_owned(),
        media_type: "tv".to_owned(),
    };
    assert!(matches!(
        sonarr(&fake).register_root_folder(&folder).await,
        Err(Failure::Refused { .. })
    ));
}

#[tokio::test]
async fn a_rejected_download_client_registration_is_refused() {
    let fake = Fake::always(Answer::reply(400, "unknown implementation"));
    assert!(matches!(
        sonarr(&fake).register_download_client(&sabnzbd()).await,
        Err(Failure::Refused { .. })
    ));
}
