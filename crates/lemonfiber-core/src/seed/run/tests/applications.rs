//! Registering each *arr with the indexer manager.

use super::*;

/// The wirings whose connection registers an \*arr into Prowlarr's app sync.
fn application_wirings(report: &crate::seed::Report) -> Vec<&crate::seed::Wiring> {
    report
        .wirings
        .iter()
        .filter(|wiring| wiring.connection.contains("indexer sync via"))
        .collect()
}

#[test]
fn the_application_kind_follows_from_the_media() {
    use crate::ports::service::ApplicationKind;
    assert_eq!(
        application_kind(&["tv".to_owned()]),
        Some(ApplicationKind::Sonarr)
    );
    assert_eq!(
        application_kind(&["movies".to_owned()]),
        Some(ApplicationKind::Radarr)
    );
    assert_eq!(
        application_kind(&["music".to_owned()]),
        Some(ApplicationKind::Lidarr)
    );
    // Bindery files books but is not one of Prowlarr's applications.
    assert!(application_kind(&["books".to_owned()]).is_none());
    // A service that files no media is not an application at all.
    assert!(application_kind(&[]).is_none());
}

#[test]
fn prowlarr_is_the_servarr_service_that_files_no_media() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    // A media-filing *arr is never the source, however reachable.
    assert!(prowlarr_source(&[arr("sonarr", 8989, "tv")], Some(project)).is_none());
    // The Servarr service with no media is, known on the network by its own
    // container name and port.
    let source = prowlarr_source(&[prowlarr()], Some(project));
    assert!(source.is_some_and(
        |source| source.network_url == "http://prowlarr:9696" && source.target.id == "prowlarr"
    ));
    // Without a project there is nowhere to read a key from.
    assert!(prowlarr_source(&[prowlarr()], None).is_none());
}

#[test]
fn no_project_directory_means_no_arrs_to_sync() {
    assert!(syncable_arrs(&[arr("sonarr", 8989, "tv")], None).is_empty());
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    let arrs = syncable_arrs(
        &[arr("sonarr", 8989, "tv"), arr("radarr", 7878, "movies")],
        Some(project),
    );
    assert_eq!(arrs.len(), 2, "each media-filing arr is syncable");
    assert!(arrs
        .iter()
        .any(|arr| arr.network_url == "http://sonarr:8989"));
}

#[tokio::test]
async fn app_sync_does_nothing_where_the_stack_has_no_prowlarr() {
    let ctx = seed_ctx(None, true, Vec::new(), None, None);
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    // Only a media-filing arr, so there is no app-sync source at all.
    let wirings =
        super::super::seed_applications(&ctx, &[arr("sonarr", 8989, "tv")], Some(project)).await;
    assert!(wirings.is_empty(), "no Prowlarr, no app sync");
}

#[tokio::test]
async fn app_sync_skips_every_arr_until_prowlarr_has_written_its_key() {
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
    // Prowlarr's key is not readable yet, so it is still starting: every
    // application is skipped for a re-run rather than failed.
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));
    let wirings = super::super::seed_applications(&ctx, &services, Some(project)).await;
    assert_eq!(wirings.len(), 1);
    assert!(wirings.iter().all(is_skipped));
}

#[tokio::test]
async fn app_sync_skips_only_the_arr_that_has_not_written_its_key() {
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
    // Prowlarr's key is readable but Sonarr's is not — Sonarr came up after
    // Prowlarr — so Sonarr's application waits while Prowlarr itself proceeds.
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(seeding())
        .with_filesystem(Arc::new(
            SeedFs::keyed(Some(SERVARR), None).only_for_prowlarr(),
        ));
    let wirings = super::super::seed_applications(&ctx, &services, Some(project)).await;
    assert_eq!(wirings.len(), 1);
    assert!(wirings.iter().all(is_skipped));
}

#[tokio::test]
async fn app_sync_registers_an_arr_whose_keys_are_all_readable() {
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
    // The seeding routes report Sonarr already registered — its baseUrl is in the
    // application list — so the connection reads back as already wired.
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(seeding())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));
    let wirings = super::super::seed_applications(&ctx, &services, Some(project)).await;
    assert_eq!(wirings.len(), 1);
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::AlreadyWired)
    );
}

/// A Prowlarr transport that starts with no applications, captures the POST
/// that registers one, and reports it on the next read — so the orchestrator's
/// write path runs end to end rather than short-circuiting to already-wired.
/// Prowlarr holding no applications until one is written, then holding it.
///
/// The registration is a write followed by a read that has to see it, so the read
/// answers an empty list and then the list with Sonarr in it. What was posted is
/// read off the transport's own record rather than a captured copy.
fn registering_prowlarr() -> Arc<Fake> {
    Fake::by_route_in_turn(vec![
        (Method::Post, "", vec![Answer::reply(201, "")]),
        (
            Method::Get,
            "",
            vec![
                Answer::reply(200, "[]"),
                Answer::reply(
                    200,
                    r#"[{"id":9,"fields":[{"name":"baseUrl","value":"http://sonarr:8989"}]}]"#,
                ),
            ],
        ),
    ])
}

#[tokio::test]
async fn app_sync_registers_an_absent_arr_and_reads_it_back() {
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let project = std::path::Path::new("/opt/lemonfiber/stack");
    let services = vec![prowlarr(), arr("sonarr", 8989, "tv")];
    // Prowlarr holds no applications, so Sonarr is genuinely written and then
    // read back — the write path a pre-populated list would hide.
    let http = registering_prowlarr();
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));

    let wirings = super::super::seed_applications(&ctx, &services, Some(project)).await;
    assert_eq!(wirings.len(), 1);
    assert_eq!(
        wirings.first().map(|wiring| &wiring.state),
        Some(&crate::seed::State::Wired),
        "an absent application is written and confirmed by read-back"
    );

    // The orchestrator built the registration for the right *arr, reaching it
    // and Prowlarr on the stack network, and posted it to Prowlarr's v1 API.
    let posted = http
        .requests()
        .into_iter()
        .find(|request| request.method == Method::Post);
    assert!(posted
        .as_ref()
        .is_some_and(|request| request.url.ends_with("/api/v1/applications")));
    let body = posted.and_then(|request| request.body).unwrap_or_default();
    assert!(
        body.contains("http://sonarr:8989"),
        "the *arr's address: {body}"
    );
    assert!(body.contains(r#""implementation":"Sonarr""#), "{body}");
    assert!(
        body.contains("http://prowlarr:9696"),
        "Prowlarr's callback url: {body}"
    );
}

#[tokio::test]
async fn seed_registers_each_arr_into_prowlarr() {
    // The whole command against the real manifest: Prowlarr and the three
    // media-filing arrs, each already registered by the seeding routes.
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    const SABNZBD: &str = "[misc]\napi_key = the-sab-key\n";
    let ctx = seed_ctx(Some(TEMP_LOG), true, Vec::new(), Some(vec![0x11; 24]), None)
        .with_http(seeding())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), Some(SABNZBD))));

    let report = seeded(dispatch(Command::Seed, &ctx).await).unwrap_or_default();
    let applications = application_wirings(&report);
    assert_eq!(
        applications.len(),
        3,
        "Sonarr, Radarr and Lidarr each registered into Prowlarr"
    );
    assert!(applications
        .iter()
        .all(|wiring| wiring.state == crate::seed::State::AlreadyWired));
}
