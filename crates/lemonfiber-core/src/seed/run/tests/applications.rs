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

#[tokio::test]
async fn app_sync_does_nothing_where_the_stack_has_no_prowlarr() {
    let ctx = seed_ctx(None, true, Vec::new(), None, None);
    // Only a media-filing arr, so nothing asks for it at all.
    let wirings =
        super::super::seed_applications(&ctx, &fillers_of(vec![arr("sonarr", 8989, "tv")])).await;
    assert!(wirings.is_empty(), "no Prowlarr, no app sync");
}

/// An indexer this machine cannot reach, or that says nowhere the curators reach it
/// back, has nothing registered into it and says nothing.
#[tokio::test]
async fn app_sync_passes_over_an_indexer_nothing_can_reach() {
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));
    let mut unpublished = prowlarr();
    unpublished.port = None;
    unpublished.listens = Some(9696);

    let wirings = super::super::seed_applications(
        &ctx,
        &fillers_of(vec![unpublished, arr("sonarr", 8989, "tv")]),
    )
    .await;

    assert!(wirings.is_empty(), "{wirings:?}");
}

/// A plugin's curator is never registered into the indexer, which would hand it the
/// indexer's own key and every indexer behind it: nothing is asked of the indexer about
/// it, the pair is said as reached by nothing with why, and the stack's own curators are
/// registered as ever.
#[tokio::test]
async fn app_sync_never_registers_a_plugin_curator() {
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let http = seeding();
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));
    let fillers = beside_a_stand_in(vec![prowlarr(), arr("sonarr", 8989, "tv")], "movies");

    let wirings = super::super::seed_applications(&ctx, &fillers).await;
    let said: Vec<crate::seed::Wiring> = super::super::connecting::unmatched(&fillers)
        .into_iter()
        .filter(|wiring| wiring.connection.starts_with("kept"))
        .collect();

    assert!(
        wirings
            .iter()
            .all(|wiring| wiring.connection.starts_with("sonarr the app")),
        "{wirings:?}"
    );
    assert!(!wirings.is_empty());
    assert!(http.requests().iter().all(|asked| {
        !asked.url.contains("kept") && !asked.body.as_deref().unwrap_or_default().contains("kept")
    }));
    assert!(
        matches!(said.as_slice(), [wiring]
            if wiring.connection == "kept the stand-in into prowlarr the app"
                && matches!(&wiring.state, crate::seed::State::Unmatched { reason }
                    if reason.contains("is a plugin's service"))),
        "{said:?}"
    );
}

/// What replacing a curator's key owes each indexer is its application held to the new
/// one, and nothing for a curator no indexer registers.
#[tokio::test]
async fn a_replaced_key_resyncs_each_indexer_that_registers_the_curator() {
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(seeding())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));
    let fillers = fillers_of(vec![prowlarr(), arr("sonarr", 8989, "tv")]);

    let resynced = super::super::resync_application(&ctx, &fillers, "sonarr").await;
    let nobody = super::super::resync_application(&ctx, &fillers, "radarr").await;

    assert_eq!(
        resynced,
        vec![(
            "prowlarr the app".to_owned(),
            crate::seed::State::AlreadyWired
        )]
    );
    assert!(nobody.is_empty());
}

/// An indexer that has not written its key yet holds no application to bring up to a
/// replaced key, so a replacement owes it nothing yet.
#[tokio::test]
async fn a_replaced_key_owes_an_indexer_with_no_key_yet_nothing() {
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));
    let fillers = fillers_of(vec![prowlarr(), arr("sonarr", 8989, "tv")]);

    let resynced = super::super::resync_application(&ctx, &fillers, "sonarr").await;

    assert!(resynced.is_empty(), "{resynced:?}");
}

/// Replacing a plugin's curator's key owes the indexer nothing: it was never registered
/// there, so there is no application to hold to the new key.
#[tokio::test]
async fn a_replaced_key_owes_the_indexer_nothing_for_a_plugin_curator() {
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let http = seeding();
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));
    let fillers = beside_a_stand_in(vec![prowlarr(), arr("sonarr", 8989, "tv")], "movies");

    let resynced = super::super::resync_application(&ctx, &fillers, "kept").await;

    assert!(resynced.is_empty(), "{resynced:?}");
    assert!(http.requests().is_empty());
}

#[tokio::test]
async fn app_sync_skips_every_arr_until_prowlarr_has_written_its_key() {
    let services = fillers_of(vec![prowlarr(), arr("sonarr", 8989, "tv")]);
    // Prowlarr's key is not readable yet, so it is still starting: every
    // application is skipped for a re-run rather than failed.
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_filesystem(Arc::new(SeedFs::keyed(None, None)));
    let wirings = super::super::seed_applications(&ctx, &services).await;
    assert_eq!(wirings.len(), 1);
    assert!(wirings.iter().all(is_skipped));
    // The key that is missing is Prowlarr's, so it is Prowlarr the report names.
    assert!(
        wirings.iter().all(|wiring| matches!(&wiring.state,
            crate::seed::State::Skipped { reason } if reason.starts_with("prowlarr the app has"))),
        "{wirings:?}"
    );
}

#[tokio::test]
async fn app_sync_skips_only_the_arr_that_has_not_written_its_key() {
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let services = fillers_of(vec![prowlarr(), arr("sonarr", 8989, "tv")]);
    // Prowlarr's key is readable but Sonarr's is not — Sonarr came up after
    // Prowlarr — so Sonarr's application waits while Prowlarr itself proceeds.
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(seeding())
        .with_filesystem(Arc::new(
            SeedFs::keyed(Some(SERVARR), None).only_for_prowlarr(),
        ));
    let wirings = super::super::seed_applications(&ctx, &services).await;
    assert_eq!(wirings.len(), 1);
    assert!(wirings.iter().all(is_skipped));
}

#[tokio::test]
async fn app_sync_registers_an_arr_whose_keys_are_all_readable() {
    const SERVARR: &str = "<Config><ApiKey>the-key</ApiKey></Config>";
    let services = fillers_of(vec![prowlarr(), arr("sonarr", 8989, "tv")]);
    // The seeding routes report Sonarr already registered — its baseUrl is in the
    // application list — so the connection reads back as already wired.
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(seeding())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));
    let wirings = super::super::seed_applications(&ctx, &services).await;
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
    let services = fillers_of(vec![prowlarr(), arr("sonarr", 8989, "tv")]);
    // Prowlarr holds no applications, so Sonarr is genuinely written and then
    // read back — the write path a pre-populated list would hide.
    let http = registering_prowlarr();
    let ctx = seed_ctx(None, true, Vec::new(), None, None)
        .with_http(http.clone())
        .with_filesystem(Arc::new(SeedFs::keyed(Some(SERVARR), None)));

    let wirings = super::super::seed_applications(&ctx, &services).await;
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
